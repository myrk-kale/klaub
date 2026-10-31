//! The flow document contract (`schema_version: 1`).
//!
//! A flow document is written by an agent (through the `klaub-flow` skill) and
//! is therefore **untrusted**. These types describe its shape; the
//! `klaub-flow` crate validates it against the snapshot before anything is shown.

use serde::{Deserialize, Serialize};

use crate::graph::{EdgeKind, EdgeOrigin};
use crate::ids::{SnapshotId, WorktreeId};

/// The current version of the flow schema.
pub const FLOW_SCHEMA_VERSION: u32 = 1;

/// A step outside the repository (a queue, a remote service, a user action).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalRef {
    /// What kind of thing it is (`http`, `queue`, `user`, ...). Text only; never a link.
    pub kind: String,
    /// Its name (`POST /login`).
    pub name: String,
}

/// One step of the explained flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStep {
    /// Unique id within the document.
    pub id: String,
    /// Path relative to the worktree.
    #[serde(default)]
    pub file_path: Option<String>,
    /// Symbol name inside the file.
    #[serde(default)]
    pub symbol: Option<String>,
    /// For steps that are not code in this repository.
    #[serde(default)]
    pub external_ref: Option<ExternalRef>,
    /// A short explanation.
    #[serde(default)]
    pub note: String,
    /// Whether the step is part of the change under review.
    #[serde(default)]
    pub changed: bool,
    /// How sure the author is. A hypothesis until the index corroborates it.
    pub confidence: EdgeOrigin,
}

/// A relation between two steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEdge {
    /// Source step id.
    pub from: String,
    /// Target step id.
    pub to: String,
    /// The relation.
    pub kind: EdgeKind,
    /// How sure the author is.
    pub confidence: EdgeOrigin,
}

/// The document as written to `.klaub/flows/<snapshot-id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowDocument {
    /// Must equal [`FLOW_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The snapshot the flow explains.
    pub snapshot_id: SnapshotId,
    /// The base commit of that snapshot.
    pub base_oid: String,
    /// The worktree of that snapshot.
    pub worktree_id: WorktreeId,
    /// A title.
    pub title: String,
    /// A one-paragraph summary.
    #[serde(default)]
    pub summary: String,
    /// Which tool wrote it (`codex`, `claude`).
    #[serde(default)]
    pub generated_by: String,
    /// Steps in reading order.
    pub steps: Vec<FlowStep>,
    /// Relations between steps (branches and cycles).
    #[serde(default)]
    pub edges: Vec<FlowEdge>,
}

/// The state of the flow for a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FlowState {
    /// A valid document for this very snapshot exists.
    Current,
    /// There is no document.
    Missing,
    /// There is a document but it failed validation.
    Invalid {
        /// What is wrong.
        reason: String,
    },
    /// The document belongs to another snapshot.
    Stale {
        /// The snapshot the document claims.
        found: String,
    },
}

impl FlowState {
    /// The label used on the CLI and in exports.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Missing => "missing",
            Self::Invalid { .. } => "invalid",
            Self::Stale { .. } => "stale",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_example() {
        let json = r#"{
            "schema_version": 1,
            "snapshot_id": "snap_abc123",
            "base_oid": "oid_da_base",
            "worktree_id": "wt_123",
            "title": "Authentication flow",
            "summary": "How a request reaches credential validation",
            "generated_by": "codex",
            "steps": [
                {"id": "request", "file_path": "api/routes/auth.py", "symbol": "login", "note": "Receives the login", "changed": false, "confidence": "resolved"},
                {"id": "validation", "file_path": "domain/auth.py", "symbol": "validate_credentials", "note": "Validates", "changed": true, "confidence": "heuristic"}
            ],
            "edges": [{"from": "request", "to": "validation", "kind": "calls", "confidence": "heuristic"}],
            "future_field": true
        }"#;
        let doc: FlowDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.steps.len(), 2);
        assert_eq!(doc.edges[0].kind, EdgeKind::Calls);
        assert_eq!(doc.steps[1].confidence, EdgeOrigin::Heuristic);
    }

    #[test]
    fn rejects_unknown_confidence_values() {
        let json = r#"{"id":"a","confidence":"certain"}"#;
        assert!(serde_json::from_str::<FlowStep>(json).is_err());
    }
}
