//! Test association and test evidence.
//!
//! "No test found" means *no association was found*; it never proves that no
//! test exists or that the code is untested.

use serde::{Deserialize, Serialize};

use crate::graph::SymbolKey;
use crate::ids::TestId;
use crate::path::RepoPath;
use crate::semantic::Confidence;
use crate::timestamp::Timestamp;

/// A test function or method.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestCase {
    /// `path::qualified_name`, the way runners identify it.
    pub id: TestId,
    /// The file that defines it.
    pub file: RepoPath,
    /// The qualified name inside the file.
    pub name: String,
    /// The line of its definition (1-based).
    pub line: u32,
}

/// How a test was linked to the code under review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkMethod {
    /// File or test name follows a convention (`test_x.py` for `x.py`).
    NameConvention,
    /// The test file imports the module under test.
    Import,
    /// The test calls the symbol by name.
    Reference,
    /// The link comes from configuration.
    Configured,
}

impl LinkMethod {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::NameConvention => "name",
            Self::Import => "import",
            Self::Reference => "reference",
            Self::Configured => "configured",
        }
    }
}

/// A heuristic association between a test and a symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestLink {
    /// The test.
    pub test: TestCase,
    /// The code it is believed to exercise.
    pub target: SymbolKey,
    /// How the link was found.
    pub method: LinkMethod,
    /// How sure the link is.
    pub confidence: Confidence,
}

/// What a test run reported for one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    /// Passed.
    Passed,
    /// Failed or errored.
    Failed,
    /// Skipped.
    Skipped,
    /// No result is available.
    NotRun,
}

/// Formats of report files Klaub can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    /// JUnit XML.
    JUnit,
    /// Coverage.py XML (Cobertura schema).
    CoveragePy,
}

/// Where a result or coverage figure came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSource {
    /// Path as given by the user or configuration.
    pub path: String,
    /// Format of the report.
    pub format: ReportFormat,
    /// File modification time, when known.
    pub modified: Option<Timestamp>,
    /// Hash of the report content.
    pub content_hash: String,
    /// The report predates the worktree state it is compared with.
    pub stale: bool,
}

/// One test result from a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    /// The test as named by the report (class name and test name).
    pub name: String,
    /// The file, when the report says.
    pub file: Option<RepoPath>,
    /// The outcome.
    pub outcome: TestOutcome,
    /// Duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Failure message, truncated.
    pub message: Option<String>,
}

/// Line coverage of one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageEntry {
    /// The file.
    pub file: RepoPath,
    /// Lines executed at least once.
    pub covered_lines: Vec<u32>,
    /// Lines never executed.
    pub missed_lines: Vec<u32>,
}

impl CoverageEntry {
    /// Fraction of measured lines that were executed, if any were measured.
    #[allow(clippy::cast_precision_loss)]
    pub fn line_rate(&self) -> Option<f32> {
        let total = self.covered_lines.len() + self.missed_lines.len();
        (total > 0).then(|| self.covered_lines.len() as f32 / total as f32)
    }
}

/// The evidence status shown for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    /// No association was found (this does not prove there are no tests).
    NoAssociationFound,
    /// Tests are associated but no result is available.
    NotRun,
    /// Associated tests passed in a report that matches the worktree.
    Passed,
    /// At least one associated test failed.
    Failed,
    /// Results exist but the report is older than the worktree state.
    Stale,
}

impl EvidenceStatus {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::NoAssociationFound => "no association found",
            Self::NotRun => "not run",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Stale => "stale",
        }
    }
}

/// All test evidence for a snapshot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TestEvidence {
    /// Heuristic associations.
    pub links: Vec<TestLink>,
    /// Results imported from reports.
    pub results: Vec<TestResult>,
    /// Coverage imported from reports.
    pub coverage: Vec<CoverageEntry>,
    /// The reports that were read.
    pub sources: Vec<ReportSource>,
}

impl TestEvidence {
    /// Links whose target is `symbol`.
    pub fn links_for<'a>(&'a self, symbol: &'a SymbolKey) -> impl Iterator<Item = &'a TestLink> {
        self.links.iter().filter(move |l| &l.target == symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_rate_needs_measured_lines() {
        let entry = CoverageEntry {
            file: RepoPath::new("a.py").unwrap(),
            covered_lines: vec![1, 2, 3],
            missed_lines: vec![4],
        };
        assert_eq!(entry.line_rate(), Some(0.75));
        let empty = CoverageEntry {
            covered_lines: vec![],
            missed_lines: vec![],
            ..entry
        };
        assert_eq!(empty.line_rate(), None);
    }
}
