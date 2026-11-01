//! Driven adapters: the implementations of the ports in [`crate::domain::ports`] that reach
//! outside the process.
//!
//! - [`git`]: snapshots of a Git worktree through the `git` executable;
//! - [`parse`]: source parsing, one grammar per language;
//! - [`storage`]: reviews, the derived-data cache, flows, configuration and files on disk;
//! - [`tmux`]: the private tmux server, the sessions and the paste into an agent's pane.
//!
//! `process` and `system` are the small ones: the clock, random identifiers, the clipboard and the
//! user's test command. Only this module and `cli` reach the file system, processes or the
//! environment; the architecture test forbids them everywhere else.

pub mod git;
pub mod parse;
pub mod storage;
pub mod tmux;

mod process;
mod system;

pub(crate) use process::{ClipboardCopy, ProcessTestRunner};
pub(crate) use system::{RandomIds, SystemClock};
