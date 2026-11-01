//! Domain services: the pure rules of Klaub.
//!
//! Each module takes domain values in and gives domain values out. None of them performs I/O or
//! names an adapter; the architecture test (`tests/architecture.rs`) fails if one does.
//!
//! - [`matching`]: which symbol of one version is which symbol of the other, and how it changed;
//! - [`graph`]: the call and dependency graph, and what a change can reach;
//! - [`rules`]: findings, signals and the explainable review queue;
//! - [`test_evidence`]: association of tests with symbols, imported reports, run requests;
//! - [`context`]: the reviewer's brief and the coverage of their review;
//! - [`flow`]: validation of the flow documents an agent writes;
//! - [`review`]: comments, anchors, feedback and exports.

pub mod context;
pub mod flow;
pub mod graph;
pub mod matching;
pub mod review;
pub mod rules;
pub mod test_evidence;
