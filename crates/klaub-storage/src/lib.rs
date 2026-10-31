//! # klaub-storage
//!
//! The persistence adapter of Klaub. It implements the ports for everything stored on disk:
//!
//! - [`JsonReviewRepository`]: reviews as versioned JSON, written atomically under an advisory lock
//!   (ADR 0006);
//! - [`SqliteCache`]: a derived-data cache in the user's cache directory that is dropped and
//!   rebuilt on any problem;
//! - [`FsFlowRepository`]: flow documents, read with size limits and without following links;
//! - [`FsFileReader`]: briefs and reports the user names, confined to the worktree;
//! - [`FsConfigSource`]: the user's `config.toml` merged with the project's `.klaub.toml`.
//!
//! Klaub's data lives in `.klaub/` inside a repository that may be untrusted, so symbolic links in
//! that path are refused rather than followed.

mod cache;
mod config;
mod files;
mod flows;
mod fsutil;
mod reviews;

pub use cache::{NoCache, SqliteCache};
pub use config::{FsConfigSource, PROJECT_CONFIG};
pub use files::FsFileReader;
pub use flows::FsFlowRepository;
pub use reviews::JsonReviewRepository;
