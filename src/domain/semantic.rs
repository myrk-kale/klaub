//! The result of comparing the symbols of two versions of a file.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::ids::{UnitId, digest_parts};
use crate::domain::language::Language;
use crate::domain::path::RepoPath;
use crate::domain::snapshot::ChangeStatus;
use crate::domain::symbol::{ParsedFile, Span, SymbolKind};

/// Default lower bound of a strong match.
pub const STRONG_MATCH: f32 = 0.85;
/// Default lower bound of a probable match.
pub const PROBABLE_MATCH: f32 = 0.65;

/// How sure a derived statement is, from 0.0 to 1.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Confidence(f32);

impl Confidence {
    /// Full certainty (exact identity matches).
    pub const CERTAIN: Self = Self(1.0);
    /// No confidence at all.
    pub const NONE: Self = Self(0.0);

    /// Clamps `value` into `0.0..=1.0`; NaN becomes 0.
    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            Self(0.0)
        } else {
            Self(value.clamp(0.0, 1.0))
        }
    }

    /// The numeric value.
    pub fn value(self) -> f32 {
        self.0
    }

    /// Classifies the confidence with the given thresholds.
    pub fn band(self, thresholds: MatchThresholds) -> MatchBand {
        if self.0 >= thresholds.strong {
            MatchBand::Strong
        } else if self.0 >= thresholds.probable {
            MatchBand::Probable
        } else {
            MatchBand::Unresolved
        }
    }
}

/// Where the bands of [`MatchBand`] begin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MatchThresholds {
    /// At or above this a match is strong.
    pub strong: f32,
    /// At or above this (and below `strong`) a match is probable.
    pub probable: f32,
}

impl Default for MatchThresholds {
    fn default() -> Self {
        Self {
            strong: STRONG_MATCH,
            probable: PROBABLE_MATCH,
        }
    }
}

/// The reliability class of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchBand {
    /// Treated as the same symbol.
    Strong,
    /// Probably the same symbol; the UI says so.
    Probable,
    /// Not claimed to be the same symbol.
    Unresolved,
}

impl MatchBand {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Probable => "probable",
            Self::Unresolved => "unresolved",
        }
    }
}

/// How a symbol changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Exists only after.
    Added,
    /// Exists only before.
    Removed,
    /// Same identity, body changed.
    Modified,
    /// Same content in another file or parent.
    Moved,
    /// Same content under another name.
    Renamed,
    /// The declaration changed (parameters, return type, decorators, visibility).
    SignatureChanged,
    /// Same identity but little of the body survives.
    Rewritten,
    /// Only whitespace or comments changed.
    FormatOnly,
    /// A match could not be established.
    Unknown,
}

impl ChangeKind {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Moved => "moved",
            Self::Renamed => "renamed",
            Self::SignatureChanged => "signature",
            Self::Rewritten => "rewritten",
            Self::FormatOnly => "format",
            Self::Unknown => "unknown",
        }
    }
}

/// A reference to a symbol inside a parsed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRef {
    /// The file.
    pub file: RepoPath,
    /// Index in [`ParsedFile::symbols`].
    pub index: usize,
    /// Dotted qualified name.
    pub qualified_name: String,
    /// Kind of the symbol.
    pub kind: SymbolKind,
    /// Normalised declaration.
    pub signature: String,
    /// Location.
    pub span: Span,
}

/// One entry of the semantic diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolChange {
    /// Stable id of this change within the snapshot.
    pub id: UnitId,
    /// What happened.
    pub kind: ChangeKind,
    /// The symbol before, when it existed.
    pub before: Option<SymbolRef>,
    /// The symbol after, when it exists.
    pub after: Option<SymbolRef>,
    /// How sure the match is (1.0 for exact identity, add and remove).
    pub confidence: Confidence,
    /// Human-readable reasons for the classification.
    pub evidence: Vec<String>,
}

impl SymbolChange {
    /// The qualified name to display (the new one when it exists).
    pub fn display_name(&self) -> &str {
        self.after
            .as_ref()
            .or(self.before.as_ref())
            .map_or("?", |s| s.qualified_name.as_str())
    }

    /// The file to display.
    pub fn file(&self) -> Option<&RepoPath> {
        self.after
            .as_ref()
            .or(self.before.as_ref())
            .map(|s| &s.file)
    }
}

/// Derives the id of a symbol-level review unit.
pub fn symbol_unit_id(file: &RepoPath, qualified_name: &str, kind: SymbolKind) -> UnitId {
    let digest = digest_parts([
        b"symbol".as_slice(),
        file.as_str().as_bytes(),
        qualified_name.as_bytes(),
        kind.label().as_bytes(),
    ]);
    UnitId::from_digest(&digest)
}

/// Derives the id of a file-level review unit.
pub fn file_unit_id(file: &RepoPath) -> UnitId {
    let digest = digest_parts([b"file".as_slice(), file.as_str().as_bytes()]);
    UnitId::from_digest(&digest)
}

/// Whether line-level or symbol-level review applies to a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticSupport {
    /// Both sides were parsed.
    Full,
    /// Only a textual review is possible; the reason says why.
    TextOnly(String),
}

/// The classification of one line in a diff view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineMark {
    /// Present and unchanged on both sides.
    Same,
    /// Present on this side and different on the other.
    Changed,
    /// Present only on the after side.
    Added,
    /// Present only on the before side.
    Removed,
}

/// An inclusive range of 1-based lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    /// First line.
    pub start: u32,
    /// Last line.
    pub end: u32,
}

/// A contiguous textual change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    /// Lines before (`None` when the hunk only adds).
    pub before: Option<LineRange>,
    /// Lines after (`None` when the hunk only removes).
    pub after: Option<LineRange>,
}

/// Per-line change marks for both sides of a file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineDiff {
    /// One mark per line before.
    pub before: Vec<LineMark>,
    /// One mark per line after.
    pub after: Vec<LineMark>,
    /// Contiguous changes in source order.
    pub hunks: Vec<Hunk>,
}

/// Everything known about one changed file after parsing and matching.
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    /// Path after the change.
    pub path: RepoPath,
    /// Path before the change, when it differs.
    pub old_path: Option<RepoPath>,
    /// How Git classified the change.
    pub status: ChangeStatus,
    /// The language, when supported.
    pub language: Option<Language>,
    /// Parse of the "before" content.
    pub before: Option<Arc<ParsedFile>>,
    /// Parse of the "after" content.
    pub after: Option<Arc<ParsedFile>>,
    /// Symbol-level changes, in source order.
    pub changes: Vec<SymbolChange>,
    /// Line-level marks for the reader.
    pub line_diff: LineDiff,
    /// Whether a semantic review is possible.
    pub support: SemanticSupport,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_is_clamped_and_banded() {
        let t = MatchThresholds::default();
        assert_eq!(Confidence::new(1.7).value(), 1.0);
        assert_eq!(Confidence::new(-1.0).value(), 0.0);
        assert_eq!(Confidence::new(f32::NAN).value(), 0.0);
        assert_eq!(Confidence::new(0.85).band(t), MatchBand::Strong);
        assert_eq!(Confidence::new(0.84).band(t), MatchBand::Probable);
        assert_eq!(Confidence::new(0.65).band(t), MatchBand::Probable);
        assert_eq!(Confidence::new(0.64).band(t), MatchBand::Unresolved);
    }

    #[test]
    fn unit_ids_depend_on_identity() {
        let a = RepoPath::new("a.py").unwrap();
        let b = RepoPath::new("b.py").unwrap();
        assert_eq!(
            symbol_unit_id(&a, "f", SymbolKind::Function),
            symbol_unit_id(&a, "f", SymbolKind::Function)
        );
        assert_ne!(
            symbol_unit_id(&a, "f", SymbolKind::Function),
            symbol_unit_id(&b, "f", SymbolKind::Function)
        );
        assert_ne!(
            symbol_unit_id(&a, "f", SymbolKind::Function),
            symbol_unit_id(&a, "f", SymbolKind::Method)
        );
        assert_ne!(file_unit_id(&a), file_unit_id(&b));
    }
}
