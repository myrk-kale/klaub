//! Helpers for tests of this and other crates: throwaway Git repositories and a fixed clock.
//!
//! Enabled with the `testing` feature. Nothing here touches the user's Git configuration:
//! identity and signing are set locally in each repository.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use klaub_core::ports::Clock;
use klaub_core::timestamp::Timestamp;

/// A clock that always returns the same instant.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub Timestamp);

impl Default for FixedClock {
    fn default() -> Self {
        Self(Timestamp::from_unix(1_793_448_000).unwrap())
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

/// A Git repository in a temporary directory, removed on drop.
#[derive(Debug)]
pub struct TempRepo {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Default for TempRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl TempRepo {
    /// Creates an empty repository on branch `main`.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        let root = dir.path().canonicalize().expect("canonical path");
        let repo = Self { dir, root };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "Klaub Test"]);
        repo.git(&["config", "user.email", "test@klaub.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.git(&["config", "core.autocrlf", "false"]);
        repo
    }

    /// The canonical worktree root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Keeps the temporary directory alive and returns its path guard (for extra files).
    pub fn temp_dir(&self) -> &Path {
        self.dir.path()
    }

    /// Writes a text file, creating parent directories.
    pub fn write(&self, relative: &str, content: &str) {
        self.write_bytes(relative, content.as_bytes());
    }

    /// Writes raw bytes, creating parent directories.
    pub fn write_bytes(&self, relative: &str, content: &[u8]) {
        let full = self.root.join(relative);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }

    /// Deletes a file.
    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.root.join(relative)).unwrap();
    }

    /// Creates a symbolic link.
    #[cfg(unix)]
    pub fn symlink(&self, relative: &str, target: &str) {
        let full = self.root.join(relative);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, full).unwrap();
    }

    /// Runs git in the repository and returns trimmed standard output; panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("git is required for these tests");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Stages everything and commits; returns the new commit id.
    pub fn commit_all(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Stages the given paths.
    pub fn stage(&self, paths: &[&str]) {
        let mut args = vec!["add", "--"];
        args.extend_from_slice(paths);
        self.git(&args);
    }
}
