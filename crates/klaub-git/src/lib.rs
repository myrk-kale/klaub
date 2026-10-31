//! # klaub-git
//!
//! The Git adapter of Klaub. It implements the
//! [`SnapshotProvider`](klaub_core::ports::SnapshotProvider) port by running the
//! `git` executable with argument arrays, `-z` output and a scrubbed environment
//! (see `docs/adr/0003-git-access-through-the-git-cli.md`).
//!
//! What it captures is described by [`klaub_core::snapshot::Scope`]: the worktree
//! (base against index, working files and untracked files), the index, the
//! unstaged changes, a branch against its merge base, or two commits. Klaub's own
//! `.klaub/` directory is never part of a snapshot.
//!
//! Repositories are untrusted input. Git is invoked with `--no-ext-diff`,
//! `--no-textconv` and `core.fsmonitor=false` so repository configuration cannot
//! run programs, and symbolic links are never followed out of the worktree.

mod classify;
mod provider;
mod raw;
mod runner;
#[cfg(feature = "testing")]
pub mod testing;

pub use provider::GitSnapshotProvider;
pub use runner::GitRunner;
