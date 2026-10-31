//! Anchors: pointing a comment at code in a way that survives the next round.
//!
//! A comment is anchored to a symbol (by qualified name and kind), to a range inside it and to the
//! text it was written about (the excerpt). Relocation tries, in order, the same symbol in the
//! same file, the same symbol elsewhere, a symbol with the same declaration, and the excerpt. It
//! never moves a comment to another function merely because line numbers coincide.

use klaub_core::ids::{UnitId, digest_parts};
use klaub_core::path::RepoPath;
use klaub_core::review::Anchor;
use klaub_core::semantic::Confidence;
use klaub_core::symbol::SymbolKind;
use klaub_core::text::collapse_whitespace;

/// Below this a relocated anchor is reported as stale and needs the human.
pub(crate) const STALE_BELOW: f32 = 0.65;

/// A symbol of the new snapshot an anchor may land on.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// File of the symbol.
    pub file: RepoPath,
    /// Dotted qualified name.
    pub qualified_name: String,
    /// Kind of the symbol.
    pub kind: SymbolKind,
    /// Normalised declaration.
    pub signature: String,
    /// The review unit id of the symbol.
    pub unit_id: UnitId,
    /// First line (1-based).
    pub start_line: u32,
    /// Last line (1-based, inclusive).
    pub end_line: u32,
    /// The symbol's source text.
    pub text: String,
}

/// How an anchor was relocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    /// Same symbol, same file.
    SameSymbol,
    /// The same symbol (or declaration) found in another place.
    Moved,
    /// Only the file is known.
    FileOnly,
    /// Nothing trustworthy was found.
    Stale,
}

/// The result of relocating one anchor.
#[derive(Debug, Clone)]
pub struct Relocation {
    /// The anchor to use from now on (the old one when stale).
    pub anchor: Anchor,
    /// How sure the relocation is.
    pub confidence: Confidence,
    /// What happened.
    pub kind: RelocationKind,
}

impl Relocation {
    /// Whether a human must pick the anchor again.
    pub fn is_stale(&self) -> bool {
        self.kind == RelocationKind::Stale || self.confidence.value() < STALE_BELOW
    }
}

/// A short hash of the anchored text with whitespace collapsed.
pub fn build_anchor_fingerprint(text: &str) -> String {
    let normalized = collapse_whitespace(text);
    digest_parts([normalized.as_bytes()])
        .chars()
        .take(16)
        .collect()
}

/// Finds `excerpt` inside `text` line by line (ignoring indentation) and returns the offset of its
/// first line and how many lines it spans.
fn find_excerpt(text: &str, excerpt: &str) -> Option<(usize, usize)> {
    let wanted: Vec<&str> = excerpt
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    (0..lines.len()).find_map(|start| {
        let mut index = start;
        for line in &wanted {
            while index < lines.len() && lines[index].is_empty() {
                index += 1;
            }
            if index >= lines.len() || lines[index] != *line {
                return None;
            }
            index += 1;
        }
        Some((start, index - start))
    })
}

fn land_on(
    anchor: &Anchor,
    candidate: &Candidate,
    confidence: f32,
    kind: RelocationKind,
) -> Relocation {
    let mut new = anchor.clone();
    new.file_path = candidate.file.clone();
    new.symbol_id = Some(candidate.unit_id.clone());
    new.symbol_kind = Some(candidate.kind);
    new.qualified_name = Some(candidate.qualified_name.clone());
    new.symbol_signature = Some(candidate.signature.clone());
    let mut confidence = confidence;
    if let Some((offset, len)) = anchor
        .excerpt
        .as_deref()
        .and_then(|e| find_excerpt(&candidate.text, e))
    {
        let first = candidate.start_line + u32::try_from(offset).unwrap_or(0);
        new.line_range = Some((
            first,
            first + u32::try_from(len.saturating_sub(1)).unwrap_or(0),
        ));
    } else {
        // The text itself is gone or changed: keep the symbol, lose the exact lines.
        new.line_range = Some((candidate.start_line, candidate.end_line));
        if anchor.excerpt.is_some() {
            confidence = confidence.min(0.75);
        }
    }
    Relocation {
        anchor: new,
        confidence: Confidence::new(confidence),
        kind,
    }
}

/// Relocates `anchor` among the symbols of a new snapshot.
///
/// `files` lists the paths that exist in the new snapshot (needed for file-level anchors).
pub fn relocate(anchor: &Anchor, candidates: &[Candidate], files: &[RepoPath]) -> Relocation {
    let stale = || Relocation {
        anchor: anchor.clone(),
        confidence: Confidence::NONE,
        kind: RelocationKind::Stale,
    };
    let Some(name) = anchor.qualified_name.as_deref() else {
        return if files.contains(&anchor.file_path) {
            Relocation {
                anchor: anchor.clone(),
                confidence: Confidence::new(0.7),
                kind: RelocationKind::FileOnly,
            }
        } else {
            stale()
        };
    };
    let same_kind = |c: &&Candidate| anchor.symbol_kind.is_none_or(|k| k == c.kind);

    if let Some(exact) = candidates
        .iter()
        .filter(same_kind)
        .find(|c| c.file == anchor.file_path && c.qualified_name == name)
    {
        let same_signature = anchor.symbol_signature.as_deref() == Some(exact.signature.as_str());
        return land_on(
            anchor,
            exact,
            if same_signature { 1.0 } else { 0.9 },
            RelocationKind::SameSymbol,
        );
    }
    let by_name: Vec<&Candidate> = candidates
        .iter()
        .filter(same_kind)
        .filter(|c| c.qualified_name == name)
        .collect();
    if let [only] = by_name.as_slice() {
        return land_on(anchor, only, 0.85, RelocationKind::Moved);
    }
    if by_name.len() > 1 {
        return stale(); // several equally good homes: the human decides
    }
    if let Some(signature) = anchor.symbol_signature.as_deref() {
        let by_signature: Vec<&Candidate> = candidates
            .iter()
            .filter(same_kind)
            .filter(|c| c.signature == signature && c.file == anchor.file_path)
            .collect();
        if let [only] = by_signature.as_slice() {
            return land_on(anchor, only, 0.75, RelocationKind::Moved);
        }
    }
    if let Some(excerpt) = anchor.excerpt.as_deref() {
        let holders: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| find_excerpt(&c.text, excerpt).is_some())
            .collect();
        if let [only] = holders.as_slice() {
            return land_on(anchor, only, 0.7, RelocationKind::Moved);
        }
    }
    stale()
}

#[cfg(test)]
mod tests {
    use super::*;
    use klaub_core::review::Version;

    fn anchor() -> Anchor {
        Anchor {
            file_path: RepoPath::new("auth.py").unwrap(),
            module_id: None,
            symbol_id: None,
            symbol_kind: Some(SymbolKind::Function),
            qualified_name: Some("validate".into()),
            symbol_signature: Some("def validate(token)".into()),
            version: Version::After,
            byte_range: None,
            line_range: Some((3, 3)),
            ast_path: vec!["validate".into()],
            fingerprint: build_anchor_fingerprint("if not token: return False"),
            excerpt: Some("if not token:\n    return False".into()),
        }
    }

    fn candidate(file: &str, name: &str, signature: &str, text: &str, start: u32) -> Candidate {
        Candidate {
            file: RepoPath::new(file).unwrap(),
            qualified_name: name.into(),
            kind: SymbolKind::Function,
            signature: signature.into(),
            unit_id: UnitId::parse(format!("unit_{name}")).unwrap(),
            start_line: start,
            end_line: start + u32::try_from(text.lines().count()).unwrap() - 1,
            text: text.into(),
        }
    }

    const BODY: &str = "def validate(token):\n    log(token)\n    if not token:\n        return False\n    return True\n";

    #[test]
    fn the_same_symbol_keeps_the_comment_on_the_same_text_even_when_lines_shift() {
        let moved_down = candidate("auth.py", "validate", "def validate(token)", BODY, 20);
        let relocation = relocate(&anchor(), &[moved_down], &[]);
        assert_eq!(relocation.kind, RelocationKind::SameSymbol);
        assert!((relocation.confidence.value() - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            relocation.anchor.line_range,
            Some((22, 23)),
            "the excerpt is found two lines below the symbol start"
        );
        assert_eq!(
            relocation.anchor.symbol_id.as_ref().unwrap().as_str(),
            "unit_validate"
        );
        assert!(!relocation.is_stale());
    }

    #[test]
    fn a_changed_signature_lowers_confidence_and_a_vanished_excerpt_keeps_only_the_symbol() {
        let changed = candidate(
            "auth.py",
            "validate",
            "def validate(token, strict)",
            "def validate(token, strict):\n    return bool(token)\n",
            1,
        );
        let relocation = relocate(&anchor(), &[changed], &[]);
        assert_eq!(relocation.kind, RelocationKind::SameSymbol);
        assert!(relocation.confidence.value() <= 0.75);
        assert_eq!(relocation.anchor.line_range, Some((1, 2)));
        assert!(
            !relocation.is_stale(),
            "still a plausible anchor, but not exact"
        );
    }

    #[test]
    fn a_symbol_moved_to_another_file_is_followed_when_unambiguous() {
        let elsewhere = candidate(
            "security/tokens.py",
            "validate",
            "def validate(token)",
            BODY,
            5,
        );
        let relocation = relocate(&anchor(), &[elsewhere], &[]);
        assert_eq!(relocation.kind, RelocationKind::Moved);
        assert_eq!(relocation.anchor.file_path.as_str(), "security/tokens.py");
        assert!(relocation.confidence.value() >= 0.8);
    }

    #[test]
    fn ambiguity_is_stale_not_a_guess() {
        let a = candidate("one.py", "validate", "def validate(token)", BODY, 1);
        let b = candidate("two.py", "validate", "def validate(token)", BODY, 1);
        let relocation = relocate(&anchor(), &[a, b], &[]);
        assert!(
            relocation.is_stale(),
            "two equally good homes: the human decides"
        );
        assert_eq!(
            relocation.anchor.file_path.as_str(),
            "auth.py",
            "the old anchor is kept"
        );
    }

    #[test]
    fn a_function_with_matching_line_numbers_but_another_name_is_not_chosen() {
        let unrelated = candidate(
            "auth.py",
            "render",
            "def render(user)",
            "def render(user):\n    return user.name\n",
            3,
        );
        assert!(relocate(&anchor(), &[unrelated], &[]).is_stale());
    }

    #[test]
    fn the_excerpt_can_find_a_renamed_function() {
        let renamed = candidate(
            "auth.py",
            "check",
            "def check(token)",
            "def check(token):\n    if not token:\n        return False\n    return True\n",
            40,
        );
        let relocation = relocate(&anchor(), &[renamed], &[]);
        assert_eq!(relocation.kind, RelocationKind::Moved);
        assert!(relocation.confidence.value() < 0.85 && !relocation.is_stale());
        assert_eq!(relocation.anchor.line_range, Some((41, 42)));
    }

    #[test]
    fn file_level_anchors_depend_on_the_file_existing() {
        let mut file_anchor = anchor();
        file_anchor.qualified_name = None;
        file_anchor.symbol_kind = None;
        let present = relocate(&file_anchor, &[], &[RepoPath::new("auth.py").unwrap()]);
        assert_eq!(present.kind, RelocationKind::FileOnly);
        assert!(!present.is_stale());
        assert!(relocate(&file_anchor, &[], &[]).is_stale());
    }

    #[test]
    fn fingerprints_ignore_whitespace() {
        assert_eq!(
            build_anchor_fingerprint("a  =  1\n"),
            build_anchor_fingerprint("a = 1")
        );
        assert_ne!(
            build_anchor_fingerprint("a = 1"),
            build_anchor_fingerprint("a = 2")
        );
    }
}
