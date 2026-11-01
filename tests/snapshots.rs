//! Behaviour of the Git adapter against real throwaway repositories.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use klaub::adapters::git::GitSnapshotProvider;
use klaub::adapters::git::testing::{FixedClock, TempRepo};
use klaub::domain::path::RepoPath;
use klaub::domain::ports::SnapshotProvider;
use klaub::domain::snapshot::{ChangeStatus, ContentKind, Drift, Scope, Snapshot, SnapshotRequest};

fn provider() -> GitSnapshotProvider {
    GitSnapshotProvider::new(Arc::new(FixedClock::default()))
}

fn request(repo: &TempRepo, scope: Scope) -> SnapshotRequest {
    SnapshotRequest {
        scope,
        ..SnapshotRequest::worktree(repo.path())
    }
}

fn capture(repo: &TempRepo, scope: Scope) -> Snapshot {
    provider().capture(&request(repo, scope)).unwrap()
}

fn paths(snapshot: &Snapshot) -> Vec<&str> {
    snapshot.files.iter().map(|f| f.path.as_str()).collect()
}

fn file<'a>(snapshot: &'a Snapshot, path: &str) -> &'a klaub::domain::snapshot::FileChange {
    snapshot
        .file(&RepoPath::new(path).unwrap())
        .unwrap_or_else(|| panic!("{path} not in {:?}", paths(snapshot)))
}

fn text(change: &klaub::domain::snapshot::FileChange, after: bool) -> Option<String> {
    let side = if after {
        change.after_text()
    } else {
        change.before_text()
    };
    side.map(|t| t.as_ref().to_owned())
}

fn base_repo() -> TempRepo {
    let repo = TempRepo::new();
    repo.write("src/app.py", "def run():\n    return 1\n");
    repo.write("src/util.py", "def helper():\n    return 2\n");
    repo.write("README.md", "# demo\n");
    repo.commit_all("initial");
    repo
}

#[test]
fn a_clean_repository_has_an_empty_snapshot() {
    let repo = base_repo();
    let snapshot = capture(&repo, Scope::Worktree);
    assert!(snapshot.files.is_empty());
    assert!(snapshot.id.as_str().starts_with("snap_"));
}

#[test]
fn the_worktree_scope_combines_staged_unstaged_untracked_and_deleted_files() {
    let repo = base_repo();
    repo.write("src/app.py", "def run():\n    return 42\n"); // unstaged edit
    repo.write("src/staged.py", "x = 1\n");
    repo.stage(&["src/staged.py"]); // staged addition
    repo.write("notes/new file.txt", "untracked\n"); // untracked, with a space in the name
    repo.remove("src/util.py"); // deleted in the worktree

    let snapshot = capture(&repo, Scope::Worktree);
    assert_eq!(
        paths(&snapshot),
        vec![
            "notes/new file.txt",
            "src/app.py",
            "src/staged.py",
            "src/util.py"
        ]
    );

    let app = file(&snapshot, "src/app.py");
    assert_eq!(app.status, ChangeStatus::Modified);
    assert_eq!(text(app, false).unwrap(), "def run():\n    return 1\n");
    assert_eq!(text(app, true).unwrap(), "def run():\n    return 42\n");
    assert_eq!(
        app.language,
        Some(klaub::domain::language::Language::Python)
    );

    assert_eq!(file(&snapshot, "src/staged.py").status, ChangeStatus::Added);
    let untracked = file(&snapshot, "notes/new file.txt");
    assert_eq!(untracked.status, ChangeStatus::Added);
    assert!(untracked.before.is_none());
    let deleted = file(&snapshot, "src/util.py");
    assert_eq!(deleted.status, ChangeStatus::Deleted);
    assert!(deleted.after.is_none());
}

#[test]
fn untracked_files_can_be_left_out() {
    let repo = base_repo();
    repo.write("scratch.py", "x = 1\n");
    let snapshot = provider()
        .capture(&SnapshotRequest {
            include_untracked: false,
            ..request(&repo, Scope::Worktree)
        })
        .unwrap();
    assert!(snapshot.files.is_empty());
}

#[test]
fn ignored_files_are_never_included() {
    let repo = base_repo();
    repo.write(".gitignore", "build/\n*.log\n");
    repo.write("build/out.js", "x\n");
    repo.write("debug.log", "x\n");
    repo.write("kept.py", "x = 1\n");
    let snapshot = capture(&repo, Scope::Worktree);
    assert_eq!(paths(&snapshot), vec![".gitignore", "kept.py"]);
}

#[test]
fn klaub_data_never_enters_a_snapshot() {
    let repo = base_repo();
    repo.write(".klaub/reviews/rev_1.json", "{}");
    repo.commit_all("track klaub data");
    repo.write(".klaub/reviews/rev_1.json", "{\"changed\":true}");
    repo.write(".klaub/flows/snap_x.json", "{}");
    repo.write("real.py", "x = 1\n");

    let snapshot = capture(&repo, Scope::Worktree);
    assert_eq!(paths(&snapshot), vec!["real.py"]);

    // Writing more review data cannot create a new snapshot, nor make the worktree look changed.
    let before = snapshot.id.clone();
    repo.write(".klaub/reviews/rev_2.json", "{}");
    assert_eq!(capture(&repo, Scope::Worktree).id, before);
    let request = request(&repo, Scope::Worktree);
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Unchanged
    );
    repo.write(".klaub/reviews/rev_3.json", "{\"more\": true}");
    repo.write(".klaub/reviews/rev_2.json", "{\"edited\": true}");
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Unchanged
    );
}

#[test]
fn renames_are_detected_with_a_similarity() {
    let repo = base_repo();
    repo.git(&["mv", "src/util.py", "src/helpers.py"]);
    repo.write(
        "src/helpers.py",
        "def helper():\n    return 2\n\ndef more():\n    return 3\n",
    );
    let snapshot = capture(&repo, Scope::Worktree);
    let renamed = file(&snapshot, "src/helpers.py");
    assert_eq!(renamed.status, ChangeStatus::Renamed);
    assert_eq!(renamed.old_path.as_ref().unwrap().as_str(), "src/util.py");
    assert!(renamed.similarity.unwrap() >= 50);
    assert!(text(renamed, false).unwrap().contains("helper"));
}

#[test]
fn staged_and_unstaged_scopes_are_disjoint() {
    let repo = base_repo();
    repo.write("src/app.py", "def run():\n    return 2\n");
    repo.stage(&["src/app.py"]);
    repo.write("src/app.py", "def run():\n    return 3\n");

    let staged = capture(&repo, Scope::Staged);
    let app = file(&staged, "src/app.py");
    assert_eq!(text(app, false).unwrap(), "def run():\n    return 1\n");
    assert_eq!(text(app, true).unwrap(), "def run():\n    return 2\n");

    let unstaged = capture(&repo, Scope::Unstaged);
    let app = file(&unstaged, "src/app.py");
    assert_eq!(text(app, false).unwrap(), "def run():\n    return 2\n");
    assert_eq!(text(app, true).unwrap(), "def run():\n    return 3\n");
}

#[test]
fn the_branch_scope_compares_against_the_merge_base() {
    let repo = base_repo();
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/feature.py", "def feature():\n    pass\n");
    repo.commit_all("feature work");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("src/main_only.py", "x = 1\n");
    repo.commit_all("main moves on");
    repo.git(&["checkout", "-q", "feature"]);

    let snapshot = provider()
        .capture(&SnapshotRequest {
            base: "main".into(),
            ..request(&repo, Scope::Branch)
        })
        .unwrap();
    assert_eq!(
        paths(&snapshot),
        vec!["src/feature.py"],
        "work that only exists on main is not part of the branch"
    );
    assert!(snapshot.target_oid.is_some());
}

#[test]
fn two_commits_can_be_compared() {
    let repo = base_repo();
    let first = repo.git(&["rev-parse", "HEAD"]);
    repo.write("src/app.py", "def run():\n    return 9\n");
    let second = repo.commit_all("second");
    let snapshot = capture(
        &repo,
        Scope::Commits {
            from: first,
            to: second.clone(),
        },
    );
    assert_eq!(paths(&snapshot), vec!["src/app.py"]);
    assert_eq!(snapshot.target_oid.as_deref(), Some(second.as_str()));
}

#[test]
fn a_repository_without_commits_shows_everything_as_added() {
    let repo = TempRepo::new();
    repo.write("a.py", "x = 1\n");
    repo.write("b.py", "y = 2\n");
    repo.stage(&["a.py"]);
    let snapshot = capture(&repo, Scope::Worktree);
    assert_eq!(paths(&snapshot), vec!["a.py", "b.py"]);
    assert!(
        snapshot
            .files
            .iter()
            .all(|f| f.status == ChangeStatus::Added)
    );
    assert!(
        provider().capture(&request(&repo, Scope::Branch)).is_err(),
        "there is no branch to compare yet"
    );
}

#[test]
fn binary_oversized_and_linked_files_are_described_not_read() {
    let repo = base_repo();
    repo.write_bytes("logo.png", &[0x89, b'P', b'N', b'G', 0, 1, 2, 3]);
    repo.write_bytes("latin1.txt", &[0x63, 0x61, 0x66, 0xe9]); // not UTF-8
    repo.write("big.txt", &"x".repeat(5000));
    repo.symlink("link-to-readme", "README.md");

    let snapshot = provider()
        .capture(&SnapshotRequest {
            max_file_bytes: 1000,
            ..request(&repo, Scope::Worktree)
        })
        .unwrap();
    let kind = |path: &str| file(&snapshot, path).after.as_ref().unwrap().kind();
    assert_eq!(kind("logo.png"), ContentKind::Binary);
    assert_eq!(kind("latin1.txt"), ContentKind::Binary);
    assert_eq!(kind("big.txt"), ContentKind::TooLarge);
    assert_eq!(kind("link-to-readme"), ContentKind::Symlink);
    assert!(!file(&snapshot, "logo.png").is_reviewable_text());
}

#[test]
fn committed_oversized_files_are_not_loaded_into_memory() {
    let repo = base_repo();
    repo.write("big.txt", &"y".repeat(5000));
    repo.commit_all("big file");
    repo.write("big.txt", &"z".repeat(6000));
    let snapshot = provider()
        .capture(&SnapshotRequest {
            max_file_bytes: 1000,
            ..request(&repo, Scope::Worktree)
        })
        .unwrap();
    let big = file(&snapshot, "big.txt");
    assert_eq!(big.before.as_ref().unwrap().kind(), ContentKind::TooLarge);
    assert_eq!(big.before.as_ref().unwrap().size(), 5000);
    assert_eq!(big.after.as_ref().unwrap().kind(), ContentKind::TooLarge);
}

#[test]
fn generated_files_are_flagged() {
    let repo = base_repo();
    repo.write("package-lock.json", "{}\n");
    repo.write(
        "src/schema.py",
        "# Code generated by tool. DO NOT EDIT.\nX = 1\n",
    );
    repo.write("src/handwritten.py", "X = 1\n");
    let snapshot = capture(&repo, Scope::Worktree);
    assert!(file(&snapshot, "package-lock.json").generated);
    assert!(file(&snapshot, "src/schema.py").generated);
    assert!(!file(&snapshot, "src/handwritten.py").generated);
}

#[test]
fn snapshot_ids_follow_the_content() {
    let repo = base_repo();
    repo.write("src/app.py", "def run():\n    return 2\n");
    let one = capture(&repo, Scope::Worktree);
    let two = capture(&repo, Scope::Worktree);
    assert_eq!(one.id, two.id);
    assert_eq!(one.drift_signature, two.drift_signature);

    repo.write("src/app.py", "def run():\n    return 3\n");
    assert_ne!(capture(&repo, Scope::Worktree).id, one.id);
}

#[test]
fn discovery_works_from_subdirectories_and_fails_outside_repositories() {
    let repo = base_repo();
    let provider = provider();
    let from_root = provider.discover(repo.path()).unwrap();
    let from_subdir = provider.discover(&repo.path().join("src")).unwrap();
    assert_eq!(from_root, from_subdir);
    assert_eq!(from_root.root, repo.path());
    assert!(from_root.id.as_str().starts_with("wt_"));

    let outside = tempfile::tempdir().unwrap();
    assert!(provider.discover(outside.path()).is_err());
}

#[test]
fn probing_detects_every_kind_of_drift() {
    let repo = base_repo();
    repo.write("src/app.py", "def run():\n    return 2\n");
    let request = request(&repo, Scope::Worktree);
    let snapshot = provider().capture(&request).unwrap();
    let probe = || provider().probe(&snapshot, &request).unwrap();
    assert_eq!(probe(), Drift::Unchanged);

    // Editing a file that is already modified keeps its git status; the probe still notices.
    std::thread::sleep(std::time::Duration::from_millis(20));
    repo.write("src/app.py", "def run():\n    return 3\n");
    assert_eq!(probe(), Drift::Changed);

    let snapshot = provider().capture(&request).unwrap();
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Unchanged
    );
    repo.write("brand_new.py", "x = 1\n");
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Changed
    );
}

#[test]
fn branch_snapshots_drift_when_head_moves() {
    let repo = base_repo();
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("f.py", "x = 1\n");
    repo.commit_all("one");
    let request = SnapshotRequest {
        base: "main".into(),
        ..request(&repo, Scope::Branch)
    };
    let snapshot = provider().capture(&request).unwrap();
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Unchanged
    );
    repo.write("g.py", "x = 2\n");
    repo.commit_all("two");
    assert_eq!(
        provider().probe(&snapshot, &request).unwrap(),
        Drift::Changed
    );
}

#[test]
fn project_files_are_listed_and_read_for_each_scope() {
    let repo = base_repo();
    repo.write(".gitignore", "ignored.py\n");
    repo.write("ignored.py", "x = 1\n");
    repo.write("src/app.py", "def run():\n    return 2\n");
    repo.write("untracked.py", "u = 1\n");
    let provider = provider();

    let worktree = capture(&repo, Scope::Worktree);
    let files = provider.list_project_files(&worktree).unwrap();
    let names: Vec<&str> = files.iter().map(RepoPath::as_str).collect();
    assert!(
        names.contains(&"src/app.py")
            && names.contains(&"untracked.py")
            && names.contains(&"README.md")
    );
    assert!(!names.contains(&"ignored.py"));
    let app = RepoPath::new("src/app.py").unwrap();
    assert_eq!(
        provider
            .read_project_file(&worktree, &app)
            .unwrap()
            .unwrap()
            .as_ref(),
        "def run():\n    return 2\n"
    );

    // The index sees the committed content, not the unstaged edit.
    let staged = capture(&repo, Scope::Staged);
    let files = provider.list_project_files(&staged).unwrap();
    assert!(!files.iter().any(|p| p.as_str() == "untracked.py"));
    assert_eq!(
        provider
            .read_project_file(&staged, &app)
            .unwrap()
            .unwrap()
            .as_ref(),
        "def run():\n    return 1\n"
    );

    // A branch snapshot reads from its commit, even after the worktree moves on.
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/app.py", "def run():\n    return 5\n");
    repo.commit_all("feature");
    let branch = provider
        .capture(&SnapshotRequest {
            base: "main".into(),
            ..request(&repo, Scope::Branch)
        })
        .unwrap();
    repo.write("src/app.py", "def run():\n    return 6\n");
    assert_eq!(
        provider
            .read_project_file(&branch, &app)
            .unwrap()
            .unwrap()
            .as_ref(),
        "def run():\n    return 5\n"
    );
}

#[test]
fn many_project_files_are_read_in_one_call_with_the_answers_of_reading_each() {
    let repo = base_repo();
    repo.write("src/app.py", "def run():\n    return 2\n");
    repo.write("src/other.py", "def other():\n    return 3\n");
    repo.write("image.bin", "\u{0}\u{1}\u{2}");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.commit_all("feature");
    let provider = provider();
    let paths: Vec<RepoPath> = ["src/app.py", "missing.py", "src/other.py", "image.bin"]
        .iter()
        .map(|p| RepoPath::new(p).unwrap())
        .collect();

    for scope in [Scope::Worktree, Scope::Staged, Scope::Branch] {
        let snapshot = provider
            .capture(&SnapshotRequest {
                base: "main".into(),
                ..request(&repo, scope.clone())
            })
            .unwrap();
        let batch = provider.read_project_files(&snapshot, &paths);
        let one_by_one: Vec<_> = paths
            .iter()
            .map(|p| provider.read_project_file(&snapshot, p).unwrap())
            .collect();
        assert_eq!(batch, one_by_one, "{scope:?}");
        assert!(batch[1].is_none(), "a missing file answers None");
        assert!(batch[0].is_some() && batch[2].is_some(), "{scope:?}");
    }
}

#[test]
fn directory_symlinks_cannot_lead_outside_the_worktree() {
    let repo = base_repo();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.py"), "SECRET = 1\n").unwrap();
    repo.symlink("escape", outside.path().to_str().unwrap());
    let snapshot = capture(&repo, Scope::Worktree);
    let leaked = provider()
        .read_project_file(&snapshot, &RepoPath::new("escape/secret.py").unwrap())
        .unwrap();
    assert!(
        leaked.is_none(),
        "a symlinked directory must not expose files outside the worktree"
    );
}

#[test]
fn option_like_revisions_are_rejected() {
    let repo = base_repo();
    let error = provider()
        .capture(&SnapshotRequest {
            base: "--output=/tmp/klaub-pwned".into(),
            ..request(&repo, Scope::Branch)
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("not a valid revision"),
        "{error}"
    );
    let error = provider()
        .capture(&request(
            &repo,
            Scope::Commits {
                from: "-p".into(),
                to: "HEAD".into(),
            },
        ))
        .unwrap_err();
    assert!(
        error.to_string().contains("not a valid revision"),
        "{error}"
    );
}

#[test]
fn repository_configuration_cannot_execute_programs() {
    let repo = base_repo();
    let marker = repo.temp_dir().join("pwned");
    let script = repo.temp_dir().join("evil.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    repo.git(&["config", "diff.external", script.to_str().unwrap()]);
    repo.git(&["config", "core.fsmonitor", script.to_str().unwrap()]);
    repo.git(&["config", "diff.evil.textconv", script.to_str().unwrap()]);
    repo.write(".gitattributes", "*.py diff=evil\n");
    repo.write("src/app.py", "def run():\n    return 123\n");

    let snapshot = capture(&repo, Scope::Worktree);
    assert!(
        snapshot
            .file(&RepoPath::new("src/app.py").unwrap())
            .is_some()
    );
    assert!(!marker.exists(), "git configuration executed a program");
}

#[test]
fn unicode_paths_round_trip() {
    let repo = base_repo();
    repo.write("docs/café ☕.md", "olá\n");
    let snapshot = capture(&repo, Scope::Worktree);
    assert_eq!(paths(&snapshot), vec!["docs/café ☕.md"]);
    assert_eq!(
        text(file(&snapshot, "docs/café ☕.md"), true).unwrap(),
        "olá\n"
    );
}
