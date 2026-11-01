//! Findings, scores and the review queue.
//!
//! Priority is an *ordering of review work*, never a verdict. Every point of a
//! score carries the reason it was given, so a reviewer can disagree with it.

use serde::{Deserialize, Serialize};

use crate::domain::graph::{EntrypointScope, ImpactScope, SymbolKey};
use crate::domain::ids::UnitId;
use crate::domain::path::RepoPath;
use crate::domain::semantic::{ChangeKind, Confidence};
use crate::domain::symbol::{Span, SymbolKind};

/// Review priority; `P0` sorts first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Review immediately.
    P0,
    /// High priority.
    P1,
    /// Normal review.
    P2,
    /// Low priority.
    P3,
}

impl Priority {
    /// The label (`P0`...`P3`).
    pub fn label(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
        }
    }

    /// Parses `P0`..`P3` (case-insensitive).
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "P0" => Some(Self::P0),
            "P1" => Some(Self::P1),
            "P2" => Some(Self::P2),
            "P3" => Some(Self::P3),
            _ => None,
        }
    }

    /// Whether the priority demands early attention (`P0` or `P1`).
    pub fn is_high(self) -> bool {
        self <= Self::P1
    }
}

/// How a finding relates to the base revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Temporal {
    /// Present after the change and absent before.
    Introduced,
    /// Present before and after.
    Preexisting,
    /// Present before and gone after.
    Removed,
    /// Could not be established (low-confidence match, missing parser, large rewrite).
    Unknown,
}

impl Temporal {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Preexisting => "preexisting",
            Self::Removed => "removed",
            Self::Unknown => "unknown",
        }
    }
}

/// A place in the code that supports a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// The file.
    pub file: RepoPath,
    /// The region, when known.
    pub span: Option<Span>,
    /// What is notable there.
    pub detail: String,
}

/// An explainable observation about the code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable rule identifier (`risk.sql_concat`).
    pub rule_id: String,
    /// One-line description.
    pub title: String,
    /// Why it matters.
    pub detail: String,
    /// Supporting locations.
    pub evidence: Vec<Evidence>,
    /// How sure the rule is.
    pub confidence: Confidence,
    /// Relation to the base revision.
    pub temporal: Temporal,
    /// Stable identity used to reconcile base and target.
    pub fingerprint: String,
    /// The symbol it concerns, if any.
    pub symbol: Option<SymbolKey>,
}

/// The parts of a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreComponent {
    /// Contract or public API change.
    Contract,
    /// Reachability: entrypoints, fan-in, scope.
    Reach,
    /// Sensitivity of the area touched.
    Criticality,
    /// Complexity of the code.
    Complexity,
    /// Technical risk markers.
    TechnicalRisk,
    /// Test evidence.
    TestSignals,
    /// Architecture rules.
    Architecture,
    /// Negative points for mitigating facts.
    Mitigation,
}

impl ScoreComponent {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::Reach => "reach",
            Self::Criticality => "criticality",
            Self::Complexity => "complexity",
            Self::TechnicalRisk => "risk",
            Self::TestSignals => "tests",
            Self::Architecture => "architecture",
            Self::Mitigation => "mitigation",
        }
    }
}

/// One itemised contribution to a score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScorePart {
    /// Which component it belongs to.
    pub component: ScoreComponent,
    /// Points added (negative for mitigations).
    pub points: i32,
    /// Why, in words.
    pub reason: String,
    /// The rule that produced it, when there is one.
    pub rule_id: Option<String>,
}

/// An itemised score.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    /// All contributions in the order they were added.
    pub parts: Vec<ScorePart>,
}

impl ScoreBreakdown {
    /// Adds a contribution; zero-point contributions are ignored.
    pub fn add(
        &mut self,
        component: ScoreComponent,
        points: i32,
        reason: impl Into<String>,
        rule_id: Option<&str>,
    ) {
        if points != 0 {
            self.parts.push(ScorePart {
                component,
                points,
                reason: reason.into(),
                rule_id: rule_id.map(str::to_owned),
            });
        }
    }

    /// The sum of all parts, never below zero.
    pub fn total(&self) -> i32 {
        self.parts.iter().map(|p| p.points).sum::<i32>().max(0)
    }

    /// The sum of one component's parts.
    pub fn component_total(&self, component: ScoreComponent) -> i32 {
        self.parts
            .iter()
            .filter(|p| p.component == component)
            .map(|p| p.points)
            .sum()
    }
}

/// Whether a review unit is a whole file or a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    /// A file (used for files without symbols, binary files and unsupported languages).
    File,
    /// A function, method, class or other symbol.
    Symbol,
}

/// The smallest thing a reviewer marks as reviewed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewUnit {
    /// Stable id within the snapshot.
    pub id: UnitId,
    /// File or symbol.
    pub kind: UnitKind,
    /// The file (after the change, or before for deleted files).
    pub file: RepoPath,
    /// The symbol, for symbol units.
    pub symbol: Option<SymbolKey>,
    /// The symbol's kind.
    pub symbol_kind: Option<SymbolKind>,
    /// What to show in lists.
    pub label: String,
    /// Whether the unit's semantic content could be analysed.
    pub semantic: bool,
}

/// Short facts shown next to a queue item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueTag {
    /// The contract (signature, export) changed.
    Contract,
    /// The file is new.
    NewFile,
    /// The file was removed.
    RemovedFile,
    /// The unit was rewritten.
    Rewrite,
    /// The unit moved or was renamed.
    Moved,
    /// Only formatting changed.
    FormatOnly,
    /// No associated test was found (which does not prove there is none).
    NoTests,
    /// A semantic comparison was not possible.
    Unknown,
    /// The file looks generated.
    Generated,
}

impl QueueTag {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::NewFile => "new",
            Self::RemovedFile => "removed",
            Self::Rewrite => "rewrite",
            Self::Moved => "moved",
            Self::FormatOnly => "format",
            Self::NoTests => "no-tests",
            Self::Unknown => "unknown",
            Self::Generated => "generated",
        }
    }
}

/// One entry of the review queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    /// What to review.
    pub unit: ReviewUnit,
    /// How the symbol changed.
    pub change: ChangeKind,
    /// Computed priority.
    pub priority: Priority,
    /// Itemised score behind the priority.
    pub score: ScoreBreakdown,
    /// Findings attached to the unit.
    pub findings: Vec<Finding>,
    /// Short facts.
    pub tags: Vec<QueueTag>,
    /// Number of entrypoints that can reach the unit.
    pub entrypoints: u32,
    /// Structural reach, when a graph was available.
    pub scope: Option<ImpactScope>,
    /// The widest entrypoint scope reaching the unit.
    pub entry_scope: Option<EntrypointScope>,
}

impl QueueItem {
    /// Findings introduced by this change.
    pub fn introduced(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.temporal == Temporal::Introduced)
    }
}

/// Counts of findings by temporal class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingDelta {
    /// Present only after.
    pub introduced: u32,
    /// Present before and after.
    pub preexisting: u32,
    /// Present only before.
    pub removed: u32,
    /// Not determinable.
    pub unknown: u32,
}

impl FindingDelta {
    /// Counts findings by their temporal class.
    pub fn of<'a>(findings: impl IntoIterator<Item = &'a Finding>) -> Self {
        let mut delta = Self::default();
        for finding in findings {
            match finding.temporal {
                Temporal::Introduced => delta.introduced += 1,
                Temporal::Preexisting => delta.preexisting += 1,
                Temporal::Removed => delta.removed += 1,
                Temporal::Unknown => delta.unknown += 1,
            }
        }
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities_sort_most_urgent_first() {
        let mut list = vec![Priority::P3, Priority::P0, Priority::P2, Priority::P1];
        list.sort();
        assert_eq!(
            list,
            vec![Priority::P0, Priority::P1, Priority::P2, Priority::P3]
        );
        assert!(Priority::P1.is_high());
        assert!(!Priority::P2.is_high());
        assert_eq!(Priority::parse("p1"), Some(Priority::P1));
        assert_eq!(Priority::parse("P9"), None);
    }

    #[test]
    fn score_is_itemised_and_never_negative() {
        let mut score = ScoreBreakdown::default();
        score.add(
            ScoreComponent::Contract,
            25,
            "signature changed",
            Some("contract.signature"),
        );
        score.add(ScoreComponent::Mitigation, -40, "format only", None);
        score.add(ScoreComponent::Reach, 0, "ignored", None);
        assert_eq!(score.parts.len(), 2);
        assert_eq!(score.total(), 0);
        assert_eq!(score.component_total(ScoreComponent::Contract), 25);
    }

    #[test]
    fn delta_counts_each_class() {
        let finding = |temporal| Finding {
            rule_id: "x".into(),
            title: String::new(),
            detail: String::new(),
            evidence: vec![],
            confidence: Confidence::CERTAIN,
            temporal,
            fingerprint: String::new(),
            symbol: None,
        };
        let findings = [
            finding(Temporal::Introduced),
            finding(Temporal::Introduced),
            finding(Temporal::Unknown),
        ];
        let delta = FindingDelta::of(&findings);
        assert_eq!(
            (
                delta.introduced,
                delta.preexisting,
                delta.removed,
                delta.unknown
            ),
            (2, 0, 0, 1)
        );
    }
}
