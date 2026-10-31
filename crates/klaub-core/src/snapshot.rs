//! Immutable snapshots of the changes under review.
//!
//! A [`Snapshot`] is what the reader shows for as long as a session lasts. Its
//! id is derived from the content, so identical changes always produce the same
//! id, and any edit produces a new one.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ids::{SnapshotId, WorktreeId, digest_parts};
use crate::language::Language;
use crate::path::RepoPath;
use crate::timestamp::Timestamp;

/// Which two states are compared.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    /// The base revision against the index plus working files plus untracked files (default).
    Worktree,
    /// The base revision against the Git index.
    Staged,
    /// The Git index against the working files.
    Unstaged,
    /// The merge base of the base revision and `HEAD` against `HEAD`.
    Branch,
    /// One commit against another.
    Commits {
        /// Origin revision.
        from: String,
        /// Destination revision.
        to: String,
    },
}

impl Scope {
    /// A short label for headers and the CLI (`worktree`, `staged`, ...).
    pub fn label(&self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Branch => "branch",
            Self::Commits { .. } => "commits",
        }
    }
}

/// What to capture.
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    /// Any path inside the repository (the worktree root is discovered from it).
    pub root: PathBuf,
    /// The comparison to make.
    pub scope: Scope,
    /// The base revision for [`Scope::Worktree`], [`Scope::Staged`] and [`Scope::Branch`].
    pub base: String,
    /// Whether untracked, non-ignored files count as added files ([`Scope::Worktree`] only).
    pub include_untracked: bool,
    /// Files larger than this are listed with metadata only.
    pub max_file_bytes: u64,
}

impl SnapshotRequest {
    /// A request for the default scope: `HEAD` against the whole worktree.
    pub fn worktree(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            scope: Scope::Worktree,
            base: "HEAD".to_owned(),
            include_untracked: true,
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

/// The identity of a Git worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeIdentity {
    /// Stable id derived from the canonical root.
    pub id: WorktreeId,
    /// Canonical worktree root.
    pub root: PathBuf,
    /// The real `.git` directory of this worktree.
    pub git_dir: PathBuf,
    /// Id of the repository (shared by all of its worktrees).
    pub repository_id: String,
}

/// How a file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    /// The file exists only after.
    Added,
    /// The file exists on both sides with different content.
    Modified,
    /// The file exists only before.
    Deleted,
    /// The file moved (and possibly changed) according to Git.
    Renamed,
    /// The file was copied from another path.
    Copied,
    /// The file type changed (for example regular file to symlink).
    TypeChanged,
}

/// What a blob holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    /// UTF-8 text with its content available.
    Text,
    /// Binary or non-UTF-8 data: only metadata is kept.
    Binary,
    /// Larger than the configured limit: only metadata is kept.
    TooLarge,
    /// A symbolic link: the text is the link target, never followed.
    Symlink,
}

/// One side of a changed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Blob {
    kind: ContentKind,
    size: u64,
    hash: String,
    #[serde(skip)]
    text: Option<Arc<str>>,
}

impl Blob {
    /// A text blob. The hash is computed from the content.
    pub fn text(content: impl Into<Arc<str>>) -> Self {
        let text: Arc<str> = content.into();
        Self {
            kind: ContentKind::Text,
            size: text.len() as u64,
            hash: blake3::hash(text.as_bytes()).to_hex().to_string(),
            text: Some(text),
        }
    }

    /// A binary (or non-UTF-8) blob identified by the hash of its bytes.
    pub fn binary(size: u64, hash: impl Into<String>) -> Self {
        Self {
            kind: ContentKind::Binary,
            size,
            hash: hash.into(),
            text: None,
        }
    }

    /// A blob that exceeded the size limit.
    pub fn too_large(size: u64, hash: impl Into<String>) -> Self {
        Self {
            kind: ContentKind::TooLarge,
            size,
            hash: hash.into(),
            text: None,
        }
    }

    /// A symbolic link whose target is `target`.
    pub fn symlink(target: &str) -> Self {
        Self {
            kind: ContentKind::Symlink,
            size: target.len() as u64,
            hash: blake3::hash(target.as_bytes()).to_hex().to_string(),
            text: Some(Arc::from(target)),
        }
    }

    /// What the blob holds.
    pub fn kind(&self) -> ContentKind {
        self.kind
    }

    /// Size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Hexadecimal content hash.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// The text, when this is a [`ContentKind::Text`] blob.
    pub fn as_text(&self) -> Option<&Arc<str>> {
        (self.kind == ContentKind::Text)
            .then_some(self.text.as_ref())
            .flatten()
    }
}

/// One changed file with both of its sides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    /// Path after the change (or the path before, for deleted files).
    pub path: RepoPath,
    /// Path before the change, when it differs.
    pub old_path: Option<RepoPath>,
    /// How the file changed.
    pub status: ChangeStatus,
    /// Git's similarity score (0–100) for renames and copies.
    pub similarity: Option<u8>,
    /// Content before, if the file existed.
    pub before: Option<Blob>,
    /// Content after, if the file exists.
    pub after: Option<Blob>,
    /// The language, when supported.
    pub language: Option<Language>,
    /// Looks generated (lock files, minified bundles, `@generated` markers).
    pub generated: bool,
}

impl FileChange {
    /// Whether both sides are readable text (or absent), so a semantic review is possible.
    pub fn is_reviewable_text(&self) -> bool {
        let ok = |blob: &Option<Blob>| blob.as_ref().is_none_or(|b| b.as_text().is_some());
        ok(&self.before) && ok(&self.after)
    }

    /// The text before, if any.
    pub fn before_text(&self) -> Option<&Arc<str>> {
        self.before.as_ref().and_then(Blob::as_text)
    }

    /// The text after, if any.
    pub fn after_text(&self) -> Option<&Arc<str>> {
        self.after.as_ref().and_then(Blob::as_text)
    }
}

/// An immutable capture of the changes under review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Content-derived id.
    pub id: SnapshotId,
    /// The worktree it was taken from.
    pub worktree: WorktreeIdentity,
    /// Object id of the base commit (the all-zero tree for an unborn branch).
    pub base_oid: String,
    /// The comparison that was made.
    pub scope: Scope,
    /// The revision requested as base (for display).
    pub base_label: String,
    /// Object id of the "after" side when it is a commit (`Branch` and `Commits` scopes);
    /// `None` when the after side is the index or the working files.
    pub target_oid: Option<String>,
    /// Opaque fingerprint of everything that can change the snapshot; compare it with a
    /// fresh one to detect drift cheaply.
    pub drift_signature: String,
    /// Changed files, sorted by path. Klaub's own `.klaub/` directory is never included.
    pub files: Vec<FileChange>,
    /// When the snapshot was captured.
    pub created_at: Timestamp,
}

impl Snapshot {
    /// Derives the snapshot id from everything that defines the change set.
    ///
    /// Two captures with the same base, scope and file contents have the same id.
    pub fn compute_id(base_oid: &str, scope: &Scope, files: &[FileChange]) -> SnapshotId {
        let mut parts: Vec<Vec<u8>> = vec![
            base_oid.as_bytes().to_vec(),
            scope.label().as_bytes().to_vec(),
        ];
        if let Scope::Commits { from, to } = scope {
            parts.push(from.as_bytes().to_vec());
            parts.push(to.as_bytes().to_vec());
        }
        let mut ordered: Vec<&FileChange> =
            files.iter().filter(|f| !f.path.is_internal()).collect();
        ordered.sort_by(|a, b| a.path.cmp(&b.path));
        for file in ordered {
            parts.push(file.path.as_str().as_bytes().to_vec());
            parts.push(format!("{:?}", file.status).into_bytes());
            parts.push(
                file.before
                    .as_ref()
                    .map_or_else(Vec::new, |b| b.hash().as_bytes().to_vec()),
            );
            parts.push(
                file.after
                    .as_ref()
                    .map_or_else(Vec::new, |b| b.hash().as_bytes().to_vec()),
            );
        }
        SnapshotId::from_digest(&digest_parts(parts.iter().map(Vec::as_slice)))
    }

    /// Looks up a changed file by its (new) path.
    pub fn file(&self, path: &RepoPath) -> Option<&FileChange> {
        self.files.iter().find(|f| &f.path == path)
    }

    /// Number of changed files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Result of probing whether the worktree still matches a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drift {
    /// Nothing relevant changed since the snapshot was captured.
    Unchanged,
    /// The worktree no longer matches the snapshot.
    Changed,
}

/// Convenience used by adapters to validate a freshly built snapshot.
pub fn ensure_sorted_unique(files: &[FileChange]) -> Result<()> {
    for pair in files.windows(2) {
        if pair[0].path >= pair[1].path {
            return Err(crate::error::Error::invalid(
                "snapshot",
                format!(
                    "files must be sorted and unique, found {} before {}",
                    pair[0].path, pair[1].path
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, before: Option<&str>, after: Option<&str>) -> FileChange {
        FileChange {
            path: RepoPath::new(path).unwrap(),
            old_path: None,
            status: match (before, after) {
                (None, Some(_)) => ChangeStatus::Added,
                (Some(_), None) => ChangeStatus::Deleted,
                _ => ChangeStatus::Modified,
            },
            similarity: None,
            before: before.map(Blob::text),
            after: after.map(Blob::text),
            language: None,
            generated: false,
        }
    }

    #[test]
    fn id_is_deterministic_and_order_independent() {
        let a = file("a.py", Some("x"), Some("y"));
        let b = file("b.py", None, Some("z"));
        let one = Snapshot::compute_id("abc", &Scope::Worktree, &[a.clone(), b.clone()]);
        let two = Snapshot::compute_id("abc", &Scope::Worktree, &[b, a]);
        assert_eq!(one, two);
        assert!(one.as_str().starts_with("snap_"));
    }

    #[test]
    fn id_changes_with_content_base_and_scope() {
        let base = Snapshot::compute_id(
            "abc",
            &Scope::Worktree,
            &[file("a.py", Some("x"), Some("y"))],
        );
        assert_ne!(
            base,
            Snapshot::compute_id(
                "abc",
                &Scope::Worktree,
                &[file("a.py", Some("x"), Some("z"))]
            )
        );
        assert_ne!(
            base,
            Snapshot::compute_id(
                "def",
                &Scope::Worktree,
                &[file("a.py", Some("x"), Some("y"))]
            )
        );
        assert_ne!(
            base,
            Snapshot::compute_id("abc", &Scope::Staged, &[file("a.py", Some("x"), Some("y"))])
        );
    }

    #[test]
    fn internal_files_never_affect_the_id() {
        let plain = Snapshot::compute_id("abc", &Scope::Worktree, &[file("a.py", None, Some("x"))]);
        let with_internal = Snapshot::compute_id(
            "abc",
            &Scope::Worktree,
            &[
                file("a.py", None, Some("x")),
                file(".klaub/reviews/rev_1.json", None, Some("{}")),
            ],
        );
        assert_eq!(plain, with_internal);
    }

    #[test]
    fn only_text_blobs_expose_text() {
        assert!(Blob::text("hi").as_text().is_some());
        assert!(Blob::binary(3, "h").as_text().is_none());
        assert!(Blob::symlink("target").as_text().is_none());
        let change = FileChange {
            after: Some(Blob::binary(3, "h")),
            ..file("a.bin", None, None)
        };
        assert!(!change.is_reviewable_text());
    }

    #[test]
    fn sorted_unique_validation() {
        let a = file("a.py", None, Some("x"));
        let b = file("b.py", None, Some("x"));
        assert!(ensure_sorted_unique(&[a.clone(), b.clone()]).is_ok());
        assert!(ensure_sorted_unique(&[b, a.clone()]).is_err());
        assert!(ensure_sorted_unique(&[a.clone(), a]).is_err());
    }
}
