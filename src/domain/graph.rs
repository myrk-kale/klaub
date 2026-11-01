//! Vocabulary shared by the call graph, the impact view and the flow contract.

use serde::{Deserialize, Serialize};

use crate::domain::path::RepoPath;

/// How trustworthy a relation is. The same three values appear in flow documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrigin {
    /// Statically unambiguous.
    Resolved,
    /// Plausible, found by name or convention.
    Heuristic,
    /// A reference whose target could not be determined.
    Unresolved,
}

impl EdgeOrigin {
    /// The weaker of two origins, used when chaining edges along a path.
    #[must_use]
    pub fn weakest(self, other: Self) -> Self {
        self.max(other)
    }

    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Heuristic => "heuristic",
            Self::Unresolved => "unresolved",
        }
    }
}

/// The relation an edge expresses. Importing does not mean calling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A call expression.
    Calls,
    /// An import of a module or name.
    Imports,
    /// A re-export.
    Exports,
    /// A trait or interface implementation.
    Implements,
    /// A class inheritance.
    Extends,
    /// Any other reference.
    References,
}

/// Whether an entrypoint is reached from outside the program or from other code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrypointScope {
    /// Route, CLI, worker, consumer, job or event handler.
    External,
    /// A public function of a module or a project-defined boundary.
    Internal,
}

/// What kind of entrypoint a symbol is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntrypointKind {
    /// An HTTP route handler.
    Http {
        /// Upper-case method (`GET`) or `ANY`.
        method: String,
        /// Route pattern as written.
        route: String,
    },
    /// A command line entry (`__main__`, `[project.scripts]`, `bin`).
    Cli,
    /// A `main` function of a program.
    Main,
    /// A background worker, consumer or job.
    Worker,
    /// A public export of a module or package.
    PublicExport,
}

/// A named symbol in the post-change state of the project.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolKey {
    /// The file defining the symbol.
    pub file: RepoPath,
    /// Dotted qualified name (`SessionService.validate`).
    pub qualified_name: String,
}

impl SymbolKey {
    /// Builds a key.
    pub fn new(file: RepoPath, qualified_name: impl Into<String>) -> Self {
        Self {
            file,
            qualified_name: qualified_name.into(),
        }
    }
}

impl std::fmt::Display for SymbolKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} › {}", self.file, self.qualified_name)
    }
}

/// One hop on a path through the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathStep {
    /// The symbol at this hop.
    pub symbol: SymbolKey,
    /// How the previous hop reaches this one.
    pub origin: EdgeOrigin,
}

/// An entrypoint with its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrypoint {
    /// The entrypoint symbol.
    pub symbol: SymbolKey,
    /// Internal or external.
    pub scope: EntrypointScope,
    /// Its kind.
    pub kind: EntrypointKind,
    /// Whether the detection is exact or a guess.
    pub origin: EdgeOrigin,
    /// Why it was recognised (`decorator @app.get("/login")`).
    pub evidence: String,
}

/// A path from an entrypoint down to the changed symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointReach {
    /// The entrypoint that can reach the changed symbol.
    pub entrypoint: Entrypoint,
    /// Hops from the entrypoint (first) to the changed symbol (last).
    pub path: Vec<PathStep>,
    /// The weakest origin along the path.
    pub origin: EdgeOrigin,
}

/// How far a change reaches, by structure rather than by call depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactScope {
    /// Callers stay within the same file.
    Local,
    /// Callers stay within the same module (directory or configured module).
    Module,
    /// At least one caller crosses a module boundary.
    CrossModule,
    /// Several modules or important entrypoints are reached.
    Global,
}

impl ImpactScope {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Module => "module",
            Self::CrossModule => "cross-module",
            Self::Global => "global",
        }
    }
}

/// How complete the graph is around a symbol. Absence of a path is not proof of absence of impact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ImpactCoverage {
    /// Edges followed that were resolved statically.
    pub resolved_edges: u32,
    /// Edges followed that were found heuristically.
    pub heuristic_edges: u32,
    /// Calls in the visited area whose target could not be determined.
    pub unresolved_calls: u32,
    /// The traversal stopped at the depth limit.
    pub truncated: bool,
}

/// All callers of a symbol at one distance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallerLevel {
    /// Distance from the changed symbol (1 = direct callers).
    pub depth: u32,
    /// Callers at this distance with the origin of the edge that led to them.
    pub callers: Vec<PathStep>,
}

/// The reverse and forward neighbourhood of a changed symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    /// The changed symbol.
    pub symbol: SymbolKey,
    /// Structural reach of the change.
    pub scope: ImpactScope,
    /// Callers by distance.
    pub callers: Vec<CallerLevel>,
    /// Entrypoints that can reach the symbol, shortest paths first.
    pub entrypoints: Vec<EntrypointReach>,
    /// Symbols the changed symbol calls (direct dependencies).
    pub callees: Vec<PathStep>,
    /// Completeness of the analysis around the symbol.
    pub coverage: ImpactCoverage,
}

impl ImpactReport {
    /// Number of direct callers.
    pub fn direct_callers(&self) -> usize {
        self.callers
            .iter()
            .find(|l| l.depth == 1)
            .map_or(0, |l| l.callers.len())
    }

    /// Number of external entrypoints that reach the symbol.
    pub fn external_entrypoints(&self) -> usize {
        self.entrypoints
            .iter()
            .filter(|e| e.entrypoint.scope == EntrypointScope::External)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weakest_origin_prefers_the_less_certain() {
        assert_eq!(
            EdgeOrigin::Resolved.weakest(EdgeOrigin::Heuristic),
            EdgeOrigin::Heuristic
        );
        assert_eq!(
            EdgeOrigin::Unresolved.weakest(EdgeOrigin::Resolved),
            EdgeOrigin::Unresolved
        );
        assert_eq!(
            EdgeOrigin::Resolved.weakest(EdgeOrigin::Resolved),
            EdgeOrigin::Resolved
        );
    }

    #[test]
    fn origin_serialises_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&EdgeOrigin::Heuristic).unwrap(),
            "\"heuristic\""
        );
    }
}
