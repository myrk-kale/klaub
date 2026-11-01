//! The ports of the hexagon: everything the application needs from the outside world.
//!
//! Each trait is implemented by exactly one production adapter and by
//! in-memory fakes in tests. Ports speak the domain's language: no adapter
//! vocabulary (git object ids aside, Git is the domain here), no library types.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::domain::config::Config;
use crate::domain::error::Result;
use crate::domain::ids::{ReviewId, SnapshotId, TestId, WorktreeId};
use crate::domain::language::Language;
use crate::domain::path::RepoPath;
use crate::domain::review::{Review, ReviewSummary};
use crate::domain::snapshot::{Drift, Snapshot, SnapshotRequest, WorktreeIdentity};
use crate::domain::symbol::{HighlightSpan, ParsedFile};
use crate::domain::timestamp::Timestamp;

/// The source of the current time.
pub trait Clock: Send + Sync {
    /// The current time.
    fn now(&self) -> Timestamp;
}

/// The source of unpredictable identifiers.
pub trait IdGenerator: Send + Sync {
    /// At least 12 random lowercase hexadecimal characters.
    fn random_hex(&self) -> String;
}

/// Captures and reads the code under review (implemented by `adapters::git`).
pub trait SnapshotProvider: Send + Sync {
    /// Finds the worktree containing `path`.
    fn discover(&self, path: &Path) -> Result<WorktreeIdentity>;

    /// Captures an immutable snapshot. `.klaub/` is never part of it.
    fn capture(&self, request: &SnapshotRequest) -> Result<Snapshot>;

    /// Lists every non-ignored file of the project on the "after" side, for call graph building.
    fn list_project_files(&self, snapshot: &Snapshot) -> Result<Vec<RepoPath>>;

    /// Reads a project file on the "after" side; `None` for binary, oversized or missing files.
    fn read_project_file(&self, snapshot: &Snapshot, path: &RepoPath) -> Result<Option<Arc<str>>>;

    /// Reads many project files at once, one answer per path in the same order. A file that cannot
    /// be read answers `None`, exactly as [`read_project_file`](Self::read_project_file) would for
    /// a missing one.
    ///
    /// The default asks one file at a time. An adapter whose reads are costly one by one (a
    /// process per file, say) should override it with a single batched read.
    fn read_project_files(&self, snapshot: &Snapshot, paths: &[RepoPath]) -> Vec<Option<Arc<str>>> {
        paths
            .iter()
            .map(|path| self.read_project_file(snapshot, path).ok().flatten())
            .collect()
    }

    /// Cheaply checks whether the worktree still matches the snapshot.
    fn probe(&self, snapshot: &Snapshot, request: &SnapshotRequest) -> Result<Drift>;
}

/// Turns source text into symbols (implemented by `adapters::parse`).
pub trait SourceParser: Send + Sync {
    /// Whether this parser handles the language.
    fn supports(&self, language: Language) -> bool;

    /// Version of the extraction logic; part of cache keys.
    fn parser_version(&self) -> u32;

    /// Parses `source`. Syntax errors do not fail the call; they set `has_syntax_errors`.
    fn parse(&self, language: Language, source: &str) -> Result<ParsedFile>;

    /// Syntax highlighting spans for the reader.
    fn highlight(&self, language: Language, source: &str) -> Result<Vec<HighlightSpan>>;
}

/// Stores reviews for one worktree (implemented by `adapters::storage`).
pub trait ReviewRepository: Send + Sync {
    /// Writes the review atomically.
    fn save(&self, review: &Review) -> Result<()>;

    /// Loads a review.
    fn load(&self, id: &ReviewId) -> Result<Review>;

    /// Lists stored reviews, newest first.
    fn list(&self) -> Result<Vec<ReviewSummary>>;

    /// The most recently updated review of a worktree, if any.
    fn latest(&self, worktree: &WorktreeId) -> Result<Option<ReviewId>>;
}

/// A flow document as read from storage, not yet validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowFile {
    /// Where it was found, relative to the worktree.
    pub path: RepoPath,
    /// Raw bytes, at most the requested limit.
    pub bytes: Vec<u8>,
}

/// Reads flow documents (implemented by `adapters::storage`).
pub trait FlowRepository: Send + Sync {
    /// The relative path where the flow of a snapshot is expected.
    fn expected_path(&self, snapshot: &SnapshotId) -> Result<RepoPath>;

    /// Reads the flow file of a snapshot.
    ///
    /// Returns `None` when it does not exist. Fails when the file is a symlink that
    /// escapes the worktree or is larger than `max_bytes`.
    fn read(&self, snapshot: &SnapshotId, max_bytes: u64) -> Result<Option<FlowFile>>;
}

/// A key of the derived-data cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// The worktree the entry belongs to.
    pub worktree: WorktreeId,
    /// Hash of the cached input (file content).
    pub content_hash: String,
    /// Version of the producer; entries of other versions never match.
    pub parser_version: u32,
    /// What is cached (`parsed_file`).
    pub kind: &'static str,
}

/// A cache for derived data that can always be recomputed (implemented by `adapters::storage`).
pub trait AnalysisCache: Send + Sync {
    /// Reads an entry.
    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>>;

    /// Writes an entry.
    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()>;
}

/// Loads configuration (implemented by `adapters::storage`).
pub trait ConfigSource: Send + Sync {
    /// Loads the user configuration merged with the project's `.klaub.toml`.
    fn load(&self, worktree_root: &Path) -> Result<Config>;
}

/// Reads user-named local files safely (implemented by `adapters::storage`).
pub trait FileReader: Send + Sync {
    /// Reads at most `max_bytes` from `path`, which must resolve inside `worktree_root`.
    ///
    /// Symbolic links that point outside the worktree are rejected.
    fn read_bounded(&self, worktree_root: &Path, path: &Path, max_bytes: u64) -> Result<Vec<u8>>;

    /// Modification time of a file inside the worktree.
    fn modified(&self, worktree_root: &Path, path: &Path) -> Result<Option<Timestamp>>;
}

/// A description of the pane that hosts the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTarget {
    /// tmux pane id (`%3`).
    pub pane_id: String,
    /// Process shown to the user for confirmation (`codex`).
    pub command: String,
    /// A short line describing the destination (`session klaub-1 · window agent · pane %3`).
    pub description: String,
}

/// The result of asking for text to be prepared in the agent's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareOutcome {
    /// The text is in the agent's input; nothing was submitted.
    Prepared,
    /// The text was not delivered; the reason is safe to show.
    Unavailable(String),
}

/// Talks to the agent that is being reviewed (implemented by `adapters::tmux`).
pub trait AgentChannel: Send + Sync {
    /// The verified destination, or `None` when there is no managed agent pane.
    fn target(&self) -> Result<Option<AgentTarget>>;

    /// Pastes `text` into the agent's input as one bracketed paste and selects the agent window.
    /// Never sends `Enter` and never clears existing input.
    fn prepare_input(&self, text: &str) -> Result<PrepareOutcome>;

    /// The last `lines` lines of what the agent's pane shows, for a [`HarnessAdapter`] to read, or
    /// `None` when the channel cannot read the screen. The text is untrusted terminal output.
    ///
    /// [`HarnessAdapter`]: crate::domain::harness::HarnessAdapter
    fn screen_tail(&self, lines: usize) -> Result<Option<String>> {
        let _ = lines;
        Ok(None)
    }
}

/// Copies text for the user (implemented by `adapters::process`).
pub trait Clipboard: Send + Sync {
    /// Copies `text` to the system clipboard or the terminal's clipboard.
    fn copy(&self, text: &str) -> Result<()>;
}

/// A shared flag that asks a long operation to stop.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A new token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A confirmed request to run tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunRequest {
    /// Program and base arguments from configuration.
    pub command: Vec<String>,
    /// Test ids appended as separate arguments.
    pub selection: Vec<TestId>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Hard time limit.
    pub timeout: Duration,
}

/// What happened when tests ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunOutcome {
    /// Exit code, if the process exited on its own.
    pub exit_code: Option<i32>,
    /// The time limit was hit.
    pub timed_out: bool,
    /// The run was cancelled.
    pub cancelled: bool,
    /// How long it ran.
    pub duration: Duration,
    /// The last part of the combined output, sanitised.
    pub output_tail: String,
}

/// Runs the user's test command (implemented by `adapters::process`).
pub trait TestRunner: Send + Sync {
    /// Runs the command without a shell. Callers must have shown the preview and obtained confirmation.
    fn run(&self, request: &TestRunRequest, cancel: &CancelToken) -> Result<TestRunOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_tokens_are_shared_between_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
