//! # klaub-app
//!
//! The application layer of Klaub. It orchestrates the pure domain services through the ports of
//! `klaub-core` and never names a concrete adapter; `klaub-cli` chooses them.
//!
//! - [`analyze`] turns a snapshot request into an immutable [`Analysis`]: parsed files, semantic
//!   changes, the call graph, test evidence and the prioritised queue.
//! - [`ReviewSession`] is a human's work on that analysis: reading code, commenting, marking units
//!   reviewed, finishing, handing the feedback to the agent, refreshing and following up. Every
//!   change is persisted at once, so a crash loses nothing that was confirmed.

mod analysis;
mod contract;
mod file_contract;
mod session;
mod signature;
mod view;

use std::sync::Arc;

use klaub_core::ports::{
    AnalysisCache, Clock, ConfigSource, FileReader, FlowRepository, IdGenerator, ReviewRepository,
    SnapshotProvider, SourceParser, TestRunner,
};

pub use analysis::{Analysis, FileEntry, UnitInfo, analyze};
pub use contract::{ContractDelta, Delta, SymbolView, inspect};
pub use file_contract::{FileContract, FileKind, inspect_file};
pub use klaub_flow::{FlowRow, layout as flow_layout};
pub use session::{FlowView, OpenOptions, Reports, ReviewOutcome, ReviewSession, TestPanel};
pub use signature::SignatureParts;
pub use view::{CodeLine, CodeScope, CodeView, HighlightedSpan};

/// The collaborators the application needs. Production wiring is in `klaub-cli`; tests use real
/// adapters over temporary directories or small fakes.
#[derive(Clone)]
pub struct Ports {
    /// Captures snapshots and reads project files.
    pub snapshots: Arc<dyn SnapshotProvider>,
    /// Turns source into symbols.
    pub parser: Arc<dyn SourceParser>,
    /// Stores reviews.
    pub reviews: Arc<dyn ReviewRepository>,
    /// Reads flow documents.
    pub flows: Arc<dyn FlowRepository>,
    /// Remembers parse results.
    pub cache: Arc<dyn AnalysisCache>,
    /// Reads files the user names.
    pub files: Arc<dyn FileReader>,
    /// Loads configuration.
    pub config: Arc<dyn ConfigSource>,
    /// The current time.
    pub clock: Arc<dyn Clock>,
    /// Random identifiers.
    pub ids: Arc<dyn IdGenerator>,
    /// Runs the user's test command, when allowed.
    pub runner: Option<Arc<dyn TestRunner>>,
}
