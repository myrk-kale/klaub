//! # klaub-core
//!
//! The domain model and the ports of Klaub: the I/O-free centre of the
//! hexagonal architecture described in `docs/architecture.md`.
//!
//! This crate contains:
//!
//! - **values** that everything else speaks in: [`ids`], [`path::RepoPath`],
//!   [`snapshot`], [`symbol`], [`semantic`], [`graph`], [`finding`], [`review`],
//!   [`testing`] and [`flow`];
//! - the **configuration schema** and its parser ([`config`]);
//! - the **ports** (traits) that adapters implement ([`ports`]);
//! - **text safety** helpers ([`text`]).
//!
//! It deliberately performs no I/O. A test (`crates/klaub-cli/tests/architecture.rs`)
//! fails if file system, process, network or environment APIs creep in.

pub mod config;
pub mod dirs;
pub mod error;
pub mod finding;
pub mod flow;
pub mod glob;
pub mod graph;
pub mod harness;
pub mod ids;
pub mod language;
pub mod path;
pub mod ports;
pub mod review;
pub mod semantic;
pub mod snapshot;
pub mod symbol;
pub mod testing;
pub mod text;
pub mod timestamp;

pub use error::{Error, Result};
