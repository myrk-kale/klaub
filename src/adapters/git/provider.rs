//! The [`SnapshotProvider`] implementation on top of the Git CLI.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::ids::{WorktreeId, digest_parts};
use crate::domain::language::Language;
use crate::domain::path::RepoPath;
use crate::domain::ports::{Clock, SnapshotProvider};
use crate::domain::snapshot::{
    Blob, ChangeStatus, Drift, FileChange, Scope, Snapshot, SnapshotRequest, WorktreeIdentity,
    ensure_sorted_unique,
};
use crate::domain::{Error, Result};

use crate::adapters::git::classify::{blob_from_bytes, looks_generated};
use crate::adapters::git::raw::{RawEntry, is_zero_sha, parse_raw_z};
use crate::adapters::git::runner::{BlobData, GitRunner};

const SUBMODULE_MODE: &str = "160000";
const SYMLINK_MODE: &str = "120000";
/// Files above this size are not hashed by streaming; size and modification time stand in.
const MAX_HASHED_BYTES: u64 = 256 * 1024 * 1024;

/// Captures snapshots of a Git worktree by running `git`.
pub struct GitSnapshotProvider {
    runner: GitRunner,
    clock: Arc<dyn Clock>,
    project_file_limit: u64,
}

impl GitSnapshotProvider {
    /// A provider that uses `git` from `PATH` and stamps snapshots with `clock`.
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            runner: GitRunner::new(),
            clock,
            project_file_limit: 2 * 1024 * 1024,
        }
    }

    /// Uses a specific `git` executable.
    #[must_use]
    pub fn with_runner(mut self, runner: GitRunner) -> Self {
        self.runner = runner;
        self
    }

    /// The runner, for callers that also need `git --version`.
    pub fn runner(&self) -> &GitRunner {
        &self.runner
    }
}

#[derive(Debug, Clone)]
struct Repo {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Side {
    Commit(String),
    Index,
    Worktree,
}

#[derive(Debug, Clone)]
struct Plan {
    base_oid: String,
    base_label: String,
    after: Side,
    diff_args: Vec<String>,
    target_oid: Option<String>,
}

/// One side of a pending file change.
#[derive(Debug, Clone)]
enum SideRef {
    Absent,
    Git { sha: String, mode: String },
    Worktree,
}

#[derive(Debug, Clone)]
struct Pending {
    old_path: Option<RepoPath>,
    status: ChangeStatus,
    similarity: Option<u8>,
    before: SideRef,
    after: SideRef,
}

struct Changes {
    raw: Vec<RawEntry>,
    untracked: Vec<RepoPath>,
}

fn reject_option_like(rev: &str) -> Result<()> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err(Error::invalid(
            "revision",
            format!("{rev:?} is not a valid revision"),
        ));
    }
    Ok(())
}

fn hex12(input: &[u8]) -> String {
    digest_parts([input]).chars().take(12).collect()
}

impl GitSnapshotProvider {
    fn open(&self, path: &Path) -> Result<Repo> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let text = self.runner.run_text(
            dir,
            [
                "rev-parse",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ],
        )?;
        let mut lines = text.lines();
        let (Some(top), Some(git_dir), Some(common)) = (lines.next(), lines.next(), lines.next())
        else {
            return Err(Error::Git(
                "unexpected output from git rev-parse".to_owned(),
            ));
        };
        let canonical = |value: &str| {
            let path = Path::new(value);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                dir.join(path)
            };
            path.canonicalize()
                .map_err(|e| Error::io(path.display(), e))
        };
        Ok(Repo {
            root: canonical(top)?,
            git_dir: canonical(git_dir)?,
            common_dir: canonical(common)?,
        })
    }

    fn identity(repo: &Repo) -> WorktreeIdentity {
        WorktreeIdentity {
            id: WorktreeId::from_digest(&digest_parts([repo.root.to_string_lossy().as_bytes()])),
            root: repo.root.clone(),
            git_dir: repo.git_dir.clone(),
            repository_id: hex12(repo.common_dir.to_string_lossy().as_bytes()),
        }
    }

    fn resolve_commit(&self, repo: &Repo, rev: &str) -> Result<String> {
        reject_option_like(rev)?;
        self.runner
            .run_text(
                &repo.root,
                [
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{rev}^{{commit}}"),
                ],
            )
            .map_err(|_| Error::not_found("revision", rev))
    }

    /// `HEAD`'s commit, or the empty tree for a repository without commits.
    fn head_or_empty(&self, repo: &Repo) -> Result<(String, bool)> {
        if let Ok(oid) = self.resolve_commit(repo, "HEAD") {
            return Ok((oid, true));
        }
        let empty = self
            .runner
            .run_text(&repo.root, ["hash-object", "-t", "tree", "/dev/null"])?;
        Ok((empty, false))
    }

    fn origin_commit(&self, repo: &Repo, base: &str, head: &(String, bool)) -> Result<String> {
        if base == "HEAD" {
            return Ok(head.0.clone());
        }
        if !head.1 {
            return Err(Error::Git(
                "this repository has no commits yet, so there is nothing to compare a base with"
                    .to_owned(),
            ));
        }
        let base_oid = self.resolve_commit(repo, base)?;
        self.runner
            .run_text(&repo.root, ["merge-base", &base_oid, &head.0])
            .map_err(|_| Error::Git(format!("{base} and HEAD have no common ancestor")))
    }

    fn plan(&self, repo: &Repo, scope: &Scope, base: &str) -> Result<Plan> {
        let head = self.head_or_empty(repo)?;
        match scope {
            Scope::Worktree | Scope::Staged => {
                let origin = self.origin_commit(repo, base, &head)?;
                let staged = matches!(scope, Scope::Staged);
                Ok(Plan {
                    base_oid: origin.clone(),
                    base_label: base.to_owned(),
                    after: if staged { Side::Index } else { Side::Worktree },
                    diff_args: if staged {
                        vec!["--cached".to_owned(), origin]
                    } else {
                        vec![origin]
                    },
                    target_oid: None,
                })
            }
            Scope::Unstaged => Ok(Plan {
                base_oid: head.0,
                base_label: "index".to_owned(),
                after: Side::Worktree,
                diff_args: Vec::new(),
                target_oid: None,
            }),
            Scope::Branch => {
                if !head.1 {
                    return Err(Error::Git("this repository has no commits yet".to_owned()));
                }
                let origin = self.origin_commit(repo, base, &head)?;
                Ok(Plan {
                    base_oid: origin.clone(),
                    base_label: base.to_owned(),
                    after: Side::Commit(head.0.clone()),
                    diff_args: vec![origin, head.0.clone()],
                    target_oid: Some(head.0),
                })
            }
            Scope::Commits { from, to } => {
                let from_oid = self.resolve_commit(repo, from)?;
                let to_oid = self.resolve_commit(repo, to)?;
                Ok(Plan {
                    base_oid: from_oid.clone(),
                    base_label: from.clone(),
                    after: Side::Commit(to_oid.clone()),
                    diff_args: vec![from_oid, to_oid.clone()],
                    target_oid: Some(to_oid),
                })
            }
        }
    }

    fn changes(
        &self,
        repo: &Repo,
        plan: &Plan,
        scope: &Scope,
        include_untracked: bool,
    ) -> Result<Changes> {
        let mut args: Vec<String> = [
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--raw",
            "-z",
            "--no-abbrev",
            "--find-renames=50%",
        ]
        .map(str::to_owned)
        .to_vec();
        args.extend(plan.diff_args.iter().cloned());
        args.push("--".to_owned());
        let output = self.runner.run(&repo.root, &args)?;
        let (mut raw, skipped) = parse_raw_z(&output)?;
        // Klaub's own data must influence neither the snapshot nor the drift signature.
        raw.retain(|entry| {
            RepoPath::new(&entry.path).is_ok_and(|p| !p.is_internal() && !p.is_within(".git"))
        });
        if skipped > 0 {
            tracing::warn!(
                skipped,
                "ignored changed files whose names are not valid UTF-8"
            );
        }

        let mut untracked = Vec::new();
        if include_untracked && matches!(scope, Scope::Worktree) {
            let output = self.runner.run(
                &repo.root,
                ["ls-files", "--others", "--exclude-standard", "-z"],
            )?;
            for name in output.split(|b| *b == 0).filter(|n| !n.is_empty()) {
                if let Some(path) = std::str::from_utf8(name)
                    .ok()
                    .and_then(|n| RepoPath::new(n).ok())
                    .filter(|p| !p.is_internal() && !p.is_within(".git"))
                {
                    untracked.push(path);
                }
            }
        }
        Ok(Changes { raw, untracked })
    }

    fn signature(repo: &Repo, plan: &Plan, scope: &Scope, changes: &Changes) -> String {
        let mut parts: Vec<Vec<u8>> = vec![
            plan.base_oid.clone().into_bytes(),
            scope.label().as_bytes().to_vec(),
        ];
        if let Some(target) = &plan.target_oid {
            parts.push(target.clone().into_bytes());
        }
        let watch_files = matches!(scope, Scope::Worktree | Scope::Unstaged);
        for entry in &changes.raw {
            parts.push(
                format!(
                    "{}:{}:{}:{}",
                    entry.status, entry.path, entry.src_sha, entry.dst_sha
                )
                .into_bytes(),
            );
            if watch_files {
                parts.push(stat_token(&repo.root, &entry.path).into_bytes());
            }
        }
        for path in &changes.untracked {
            parts.push(format!("?{path}").into_bytes());
            parts.push(stat_token(&repo.root, path.as_str()).into_bytes());
        }
        digest_parts(parts.iter().map(Vec::as_slice))
    }

    fn pending_from_raw(
        raw: Vec<RawEntry>,
        untracked: Vec<RepoPath>,
        plan: &Plan,
    ) -> BTreeMap<RepoPath, Pending> {
        let mut map = BTreeMap::new();
        for entry in raw {
            let Ok(path) = RepoPath::new(&entry.path) else {
                tracing::warn!("ignored a changed file whose name cannot be represented");
                continue;
            };
            if path.is_internal() || path.is_within(".git") {
                continue;
            }
            let old_path = entry
                .old_path
                .as_deref()
                .and_then(|p| RepoPath::new(p).ok());
            let status = match entry.status {
                'A' => ChangeStatus::Added,
                'D' => ChangeStatus::Deleted,
                'R' => ChangeStatus::Renamed,
                'C' => ChangeStatus::Copied,
                'T' => ChangeStatus::TypeChanged,
                _ => ChangeStatus::Modified,
            };
            let before = if entry.before_absent() {
                SideRef::Absent
            } else {
                SideRef::Git {
                    sha: entry.src_sha.clone(),
                    mode: entry.src_mode.clone(),
                }
            };
            let after = if entry.after_absent() && entry.status != 'U' {
                SideRef::Absent
            } else if is_zero_sha(&entry.dst_sha)
                || matches!(plan.after, Side::Worktree) && entry.status == 'U'
            {
                SideRef::Worktree
            } else {
                SideRef::Git {
                    sha: entry.dst_sha.clone(),
                    mode: entry.dst_mode.clone(),
                }
            };
            map.insert(
                path,
                Pending {
                    old_path,
                    status,
                    similarity: entry.score,
                    before,
                    after,
                },
            );
        }
        for path in untracked {
            if path.is_internal() || path.is_within(".git") || map.contains_key(&path) {
                continue;
            }
            map.insert(
                path,
                Pending {
                    old_path: None,
                    status: ChangeStatus::Added,
                    similarity: None,
                    before: SideRef::Absent,
                    after: SideRef::Worktree,
                },
            );
        }
        map
    }

    fn build_blob(
        side: &SideRef,
        fetched: &std::collections::HashMap<String, BlobData>,
        root: &Path,
        path: &RepoPath,
        limit: u64,
    ) -> Result<Option<Blob>> {
        match side {
            SideRef::Absent => Ok(None),
            SideRef::Worktree => worktree_blob(root, path, limit),
            SideRef::Git { sha, mode } => Ok(Some(if mode == SUBMODULE_MODE {
                Blob::binary(0, sha.clone())
            } else {
                match fetched.get(sha) {
                    None => Blob::binary(0, sha.clone()),
                    Some(BlobData::TooLarge { size }) => Blob::too_large(*size, sha.clone()),
                    Some(BlobData::Bytes(bytes)) if mode == SYMLINK_MODE => {
                        Blob::symlink(&String::from_utf8_lossy(bytes))
                    }
                    Some(BlobData::Bytes(bytes)) => blob_from_bytes(bytes),
                }
            })),
        }
    }
}

fn stat_token(root: &Path, relative: &str) -> String {
    match fs::symlink_metadata(root.join(relative)) {
        Ok(metadata) => {
            let nanos = metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{}:{nanos}", metadata.len())
        }
        Err(_) => "missing".to_owned(),
    }
}

/// Resolves `path` below `root` without following symbolic links that leave the worktree.
///
/// The final component may itself be a symbolic link (it is reported, never followed);
/// every directory above it is canonicalised and must stay inside `root`.
fn safe_path(root: &Path, path: &RepoPath) -> Option<PathBuf> {
    let full = path.to_path(root);
    let parent = full.parent()?.canonicalize().ok()?;
    let name = full.file_name()?;
    parent.starts_with(root).then(|| parent.join(name))
}

fn worktree_blob(root: &Path, path: &RepoPath, limit: u64) -> Result<Option<Blob>> {
    let Some(full) = safe_path(root, path) else {
        return Ok(None);
    };
    let metadata = match fs::symlink_metadata(&full) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(full.display(), e)),
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target = fs::read_link(&full).map_err(|e| Error::io(full.display(), e))?;
        return Ok(Some(Blob::symlink(&target.to_string_lossy())));
    }
    if !file_type.is_file() {
        // Directories (nested repositories, submodules) have no reviewable content.
        return Ok(Some(Blob::binary(0, "directory")));
    }
    let size = metadata.len();
    if size > limit {
        let hash = if size <= MAX_HASHED_BYTES {
            let mut hasher = blake3::Hasher::new();
            let mut file = fs::File::open(&full).map_err(|e| Error::io(full.display(), e))?;
            let mut buffer = vec![0_u8; 64 * 1024];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|e| Error::io(full.display(), e))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            hasher.finalize().to_hex().to_string()
        } else {
            format!("size-mtime:{}", stat_token(root, path.as_str()))
        };
        return Ok(Some(Blob::too_large(size, hash)));
    }
    let bytes = fs::read(&full).map_err(|e| Error::io(full.display(), e))?;
    Ok(Some(blob_from_bytes(&bytes)))
}

impl SnapshotProvider for GitSnapshotProvider {
    fn discover(&self, path: &Path) -> Result<WorktreeIdentity> {
        Ok(Self::identity(&self.open(path)?))
    }

    fn capture(&self, request: &SnapshotRequest) -> Result<Snapshot> {
        let repo = self.open(&request.root)?;
        let plan = self.plan(&repo, &request.scope, &request.base)?;
        let changes = self.changes(&repo, &plan, &request.scope, request.include_untracked)?;
        let signature = Self::signature(&repo, &plan, &request.scope, &changes);
        let Changes { raw, untracked } = changes;
        let pending = Self::pending_from_raw(raw, untracked, &plan);

        let wanted: Vec<String> = pending
            .values()
            .flat_map(|p| [&p.before, &p.after])
            .filter_map(|side| match side {
                SideRef::Git { sha, mode } if mode != SUBMODULE_MODE => Some(sha.clone()),
                _ => None,
            })
            .collect();
        let fetched = self
            .runner
            .read_blobs(&repo.root, &wanted, request.max_file_bytes)?;

        let mut files = Vec::with_capacity(pending.len());
        for (path, item) in pending {
            let before = Self::build_blob(
                &item.before,
                &fetched,
                &repo.root,
                &path,
                request.max_file_bytes,
            )?;
            let after = Self::build_blob(
                &item.after,
                &fetched,
                &repo.root,
                &path,
                request.max_file_bytes,
            )?;
            let status = if after.is_none() && before.is_some() {
                ChangeStatus::Deleted
            } else {
                item.status
            };
            if before.is_none() && after.is_none() {
                continue;
            }
            let head_text = after
                .as_ref()
                .or(before.as_ref())
                .and_then(|b| b.as_text())
                .map(|t| t.as_ref().to_owned());
            files.push(FileChange {
                language: Language::from_path(&path),
                generated: looks_generated(&path, head_text.as_deref()),
                old_path: item.old_path,
                similarity: item.similarity,
                before,
                after,
                status,
                path,
            });
        }
        ensure_sorted_unique(&files)?;

        Ok(Snapshot {
            id: Snapshot::compute_id(&plan.base_oid, &request.scope, &files),
            worktree: Self::identity(&repo),
            base_oid: plan.base_oid,
            scope: request.scope.clone(),
            base_label: plan.base_label,
            target_oid: plan.target_oid,
            drift_signature: signature,
            files,
            created_at: self.clock.now(),
        })
    }

    fn list_project_files(&self, snapshot: &Snapshot) -> Result<Vec<RepoPath>> {
        let root = &snapshot.worktree.root;
        let output = match (&snapshot.scope, &snapshot.target_oid) {
            (_, Some(target)) => self.runner.run(
                root,
                ["ls-tree", "-r", "-z", "--name-only", target.as_str()],
            )?,
            (Scope::Staged, None) => self.runner.run(root, ["ls-files", "-z", "--cached"])?,
            _ => self.runner.run(
                root,
                [
                    "ls-files",
                    "-z",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                ],
            )?,
        };
        let on_disk = snapshot.target_oid.is_none() && !matches!(snapshot.scope, Scope::Staged);
        let mut files: Vec<RepoPath> = output
            .split(|b| *b == 0)
            .filter(|n| !n.is_empty())
            .filter_map(|n| std::str::from_utf8(n).ok())
            .filter_map(|n| RepoPath::new(n).ok())
            .filter(|p| !p.is_internal() && !p.is_within(".git"))
            .filter(|p| !on_disk || fs::symlink_metadata(p.to_path(root)).is_ok())
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn read_project_file(&self, snapshot: &Snapshot, path: &RepoPath) -> Result<Option<Arc<str>>> {
        let root = &snapshot.worktree.root;
        let blob = match (&snapshot.scope, &snapshot.target_oid) {
            (_, Some(target)) => self.read_spec(root, &format!("{target}:{path}"))?,
            (Scope::Staged, None) => self.read_spec(root, &format!(":{path}"))?,
            _ => worktree_blob(root, path, self.project_file_limit)?,
        };
        Ok(blob.and_then(|b| b.as_text().cloned()))
    }

    fn read_project_files(&self, snapshot: &Snapshot, paths: &[RepoPath]) -> Vec<Option<Arc<str>>> {
        let root = &snapshot.worktree.root;
        let spec_of = |path: &RepoPath| match (&snapshot.scope, &snapshot.target_oid) {
            (_, Some(target)) => Some(format!("{target}:{path}")),
            (Scope::Staged, None) => Some(format!(":{path}")),
            _ => None,
        };
        // Files that live in Git objects are fetched by one `git cat-file --batch` instead of one
        // process per file, which is what made a branch or commit review of a large project slow.
        let specs: Vec<String> = paths.iter().filter_map(spec_of).collect();
        let fetched = if specs.is_empty() {
            None
        } else {
            self.runner
                .read_blobs(root, &specs, self.project_file_limit)
                .ok()
        };
        paths
            .iter()
            .map(|path| match spec_of(path) {
                Some(spec) => match fetched.as_ref()?.get(&spec) {
                    Some(BlobData::Bytes(bytes)) => blob_from_bytes(bytes).as_text().cloned(),
                    Some(BlobData::TooLarge { .. }) | None => None,
                },
                None => worktree_blob(root, path, self.project_file_limit)
                    .ok()
                    .flatten()
                    .and_then(|b| b.as_text().cloned()),
            })
            .collect()
    }

    fn probe(&self, snapshot: &Snapshot, request: &SnapshotRequest) -> Result<Drift> {
        let repo = self.open(&snapshot.worktree.root)?;
        let plan = self.plan(&repo, &snapshot.scope, &snapshot.base_label)?;
        let changes = self.changes(&repo, &plan, &snapshot.scope, request.include_untracked)?;
        let signature = Self::signature(&repo, &plan, &snapshot.scope, &changes);
        Ok(if signature == snapshot.drift_signature {
            Drift::Unchanged
        } else {
            Drift::Changed
        })
    }
}

impl GitSnapshotProvider {
    fn read_spec(&self, root: &Path, spec: &str) -> Result<Option<Blob>> {
        let fetched = self
            .runner
            .read_blobs(root, &[spec.to_owned()], self.project_file_limit)?;
        Ok(match fetched.get(spec) {
            Some(BlobData::Bytes(bytes)) => Some(blob_from_bytes(bytes)),
            Some(BlobData::TooLarge { size }) => Some(Blob::too_large(*size, spec)),
            None => None,
        })
    }
}
