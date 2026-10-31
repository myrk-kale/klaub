//! What changed about one symbol's contract, stated from evidence the analysis already has.
//!
//! A diff shows that lines moved. This module answers the narrower question a reviewer actually has
//! when they open one changed symbol: did its declaration change, who can reach it now, and what
//! does the change touch. [`inspect`] is a pure comparison. It reads a [`SymbolChange`], the two
//! [`Symbol`]s it points at when they were parsed, and the symbol's [`ImpactReport`], and it writes
//! nothing and asks nothing of the file system.
//!
//! What it deliberately does not do is infer. Signatures arrive as one normalised string per
//! language, and splitting one into parameters and a return type would mean guessing at each
//! language's grammar, so those fields stay empty and the whole signature is kept instead. Effects
//! are named only when a parser already recorded a [`MarkerKind`] on the symbol; a call that merely
//! looks like it writes a file is not one. Anything the inputs cannot answer is reported as the word
//! "unknown" inside the evidence, never as a value invented to fill a field.

use klaub_core::graph::ImpactReport;
use klaub_core::ids::UnitId;
use klaub_core::semantic::{ChangeKind, Confidence, SymbolChange};
use klaub_core::symbol::{MarkerKind, Symbol, Visibility};

/// How many direct callers are named. The rest are counted, not listed.
const MAX_AFFECTED: usize = 20;

/// One symbol's contract, compared across the change.
///
/// Every field comes from the inputs of [`inspect`]. An empty `parameters` or `returns` means the
/// signature could not be split reliably, not that the symbol has none; the whole declaration is in
/// `before_signature` and `after_signature`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractDelta {
    /// The review unit this comparison belongs to.
    pub unit: UnitId,
    /// The qualified name shown for the change (the new one when the symbol still exists).
    pub name: String,
    /// How the symbol was classified.
    pub change_kind: ChangeKind,
    /// How sure that classification is.
    pub confidence: Confidence,
    /// The normalised declaration before the change, when the symbol existed.
    pub before_signature: Option<String>,
    /// The normalised declaration after the change, when the symbol exists.
    pub after_signature: Option<String>,
    /// Who could use the symbol before, when that was recorded.
    pub visibility_before: Option<String>,
    /// Who can use the symbol after, when that was recorded.
    pub visibility_after: Option<String>,
    /// Parameters taken out of the signature. Empty: no language-specific split is attempted.
    pub parameters: Vec<String>,
    /// The return type taken out of the signature. Empty for the same reason as `parameters`.
    pub returns: Vec<String>,
    /// Side effects a parser's risk marker already recorded, named as observed.
    pub effects: Vec<String>,
    /// Qualified names of the direct callers, capped at 20.
    pub affected: Vec<String>,
    /// The change's own reasons, plus a note wherever a fact could not be established.
    pub evidence: Vec<String>,
}

/// Compares one symbol's contract.
///
/// `before_symbol` and `after_symbol` are the parsed symbols the change's references point at.
/// Either may be absent: a symbol that was added has no before, one that was removed has no after,
/// and a reference whose parse is missing has neither. `impact` is absent when the call graph does
/// not know the symbol; that is recorded as unknown rather than as "nothing calls this".
pub fn inspect(
    change: &SymbolChange,
    before_symbol: Option<&Symbol>,
    after_symbol: Option<&Symbol>,
    impact: Option<&ImpactReport>,
) -> ContractDelta {
    let mut evidence = change.evidence.clone();
    let (parameters, returns) = split_signature(before_symbol, after_symbol, &mut evidence);
    let affected = affected_callers(impact, &mut evidence);
    if change.before.is_none() {
        evidence.push("no symbol before the change; nothing to compare against".to_owned());
    }
    if change.after.is_none() {
        evidence.push("no symbol after the change; the contract no longer exists".to_owned());
    }

    ContractDelta {
        unit: change.id.clone(),
        name: change.display_name().to_owned(),
        change_kind: change.kind,
        confidence: change.confidence,
        before_signature: signature_of(
            change.before.as_ref().map(|r| r.signature.as_str()),
            before_symbol,
        ),
        after_signature: signature_of(
            change.after.as_ref().map(|r| r.signature.as_str()),
            after_symbol,
        ),
        visibility_before: before_symbol.map(|s| visibility_label(s.visibility).to_owned()),
        visibility_after: after_symbol.map(|s| visibility_label(s.visibility).to_owned()),
        parameters,
        returns,
        effects: effects_of(before_symbol, after_symbol),
        affected,
        evidence,
    }
}

/// The declaration to show: the parsed symbol's when it was reached, otherwise the reference's.
fn signature_of(reference: Option<&str>, symbol: Option<&Symbol>) -> Option<String> {
    symbol
        .map(|s| s.signature.clone())
        .or_else(|| reference.map(str::to_owned))
}

/// Visibility as a word. `Unknown` is said as "unknown", the same word the evidence uses.
fn visibility_label(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Protected => "protected",
        Visibility::Private => "private",
        Visibility::Internal => "internal",
        Visibility::Unknown => "unknown",
    }
}

/// Parameters and return type, left empty on purpose.
///
/// A signature is one normalised string whose shape depends on the language (`def f(a) -> b`,
/// `fn f(a) -> b`, `function f(a): b`). Cutting it at the parentheses would misread decorators,
/// where-clauses and return types written before the name, so nothing is split and the whole
/// signature is what the caller reads.
fn split_signature(
    before: Option<&Symbol>,
    after: Option<&Symbol>,
    evidence: &mut Vec<String>,
) -> (Vec<String>, Vec<String>) {
    let changed = match (before, after) {
        (Some(before), Some(after)) => before.signature != after.signature,
        (None, None) => false,
        _ => true,
    };
    if changed {
        evidence.push(
            "parameters and return type unknown: the signature is kept whole, not split per language"
                .to_owned(),
        );
    }
    (Vec::new(), Vec::new())
}

/// Risk markers either side carries, named once, in a stable order.
fn effects_of(before: Option<&Symbol>, after: Option<&Symbol>) -> Vec<String> {
    let mut kinds: Vec<MarkerKind> = Vec::new();
    for symbol in [before, after].into_iter().flatten() {
        for marker in &symbol.features.markers {
            if !kinds.contains(&marker.kind) {
                kinds.push(marker.kind);
            }
        }
    }
    kinds.sort_unstable();
    kinds
        .iter()
        .map(|kind| effect_line(*kind).to_owned())
        .collect()
}

/// What a marker says was observed. The wording names the observation, not a guessed consequence.
fn effect_line(kind: MarkerKind) -> &'static str {
    match kind {
        MarkerKind::SwallowedError => "swallows an error",
        MarkerKind::RawSqlConcat => "concatenates a query string",
        MarkerKind::SecretLiteral => "holds a string that looks like a secret",
        MarkerKind::CallInLoop => "calls into I/O inside a loop",
        MarkerKind::DynamicExecution => "executes code dynamically",
        MarkerKind::ShellExecution => "invokes a shell",
        MarkerKind::UnsafeBlock => "contains an unsafe block",
        MarkerKind::VerificationDisabled => "turns verification off",
    }
}

/// Qualified names of the direct callers, or a note that the graph has no answer.
fn affected_callers(impact: Option<&ImpactReport>, evidence: &mut Vec<String>) -> Vec<String> {
    let Some(impact) = impact else {
        evidence.push(
            "affected callers unknown: the call graph has no entry for this symbol".to_owned(),
        );
        return Vec::new();
    };
    let direct = impact
        .callers
        .iter()
        .find(|level| level.depth == 1)
        .map(|level| level.callers.as_slice())
        .unwrap_or_default();
    if direct.is_empty() {
        evidence.push("no direct caller in the call graph".to_owned());
        return Vec::new();
    }
    let mut names: Vec<String> = direct
        .iter()
        .map(|step| step.symbol.qualified_name.clone())
        .collect();
    names.sort();
    names.dedup();
    if names.len() > MAX_AFFECTED {
        let hidden = names.len() - MAX_AFFECTED;
        names.truncate(MAX_AFFECTED);
        evidence.push(format!("{hidden} more direct caller(s) not listed"));
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    use klaub_core::graph::{
        CallerLevel, EdgeOrigin, ImpactCoverage, ImpactScope, PathStep, SymbolKey,
    };
    use klaub_core::path::RepoPath;
    use klaub_core::semantic::{SymbolRef, symbol_unit_id};
    use klaub_core::symbol::{Fingerprint, Marker, Span, SymbolFeatures, SymbolKind};

    fn path() -> RepoPath {
        RepoPath::new("service.py").unwrap()
    }

    fn span() -> Span {
        Span {
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            end_line: 3,
        }
    }

    fn reference(signature: &str) -> SymbolRef {
        SymbolRef {
            file: path(),
            index: 1,
            qualified_name: "lookup".to_owned(),
            kind: SymbolKind::Function,
            signature: signature.to_owned(),
            span: span(),
        }
    }

    fn symbol(signature: &str, visibility: Visibility, markers: Vec<Marker>) -> Symbol {
        Symbol {
            name: "lookup".to_owned(),
            qualified_name: "lookup".to_owned(),
            kind: SymbolKind::Function,
            signature: signature.to_owned(),
            visibility,
            span: span(),
            parent: Some(0),
            decorators: Vec::new(),
            fingerprint: Fingerprint::default(),
            features: SymbolFeatures {
                markers,
                ..SymbolFeatures::default()
            },
        }
    }

    fn marker(kind: MarkerKind) -> Marker {
        Marker {
            kind,
            line: 2,
            detail: "observed".to_owned(),
        }
    }

    fn change(
        kind: ChangeKind,
        before: Option<SymbolRef>,
        after: Option<SymbolRef>,
    ) -> SymbolChange {
        let id_of = before.as_ref().or(after.as_ref()).unwrap();
        SymbolChange {
            id: symbol_unit_id(&id_of.file, &id_of.qualified_name, id_of.kind),
            kind,
            before,
            after,
            confidence: Confidence::CERTAIN,
            evidence: vec!["signature text differs".to_owned()],
        }
    }

    fn impact(callers: &[&str]) -> ImpactReport {
        ImpactReport {
            symbol: SymbolKey::new(path(), "lookup"),
            scope: ImpactScope::Local,
            callers: vec![CallerLevel {
                depth: 1,
                callers: callers
                    .iter()
                    .map(|name| PathStep {
                        symbol: SymbolKey::new(path(), (*name).to_owned()),
                        origin: EdgeOrigin::Resolved,
                    })
                    .collect(),
            }],
            entrypoints: Vec::new(),
            callees: Vec::new(),
            coverage: ImpactCoverage::default(),
        }
    }

    #[test]
    fn a_changed_signature_is_kept_whole() {
        let before = symbol("def lookup(name)", Visibility::Public, Vec::new());
        let after = symbol("def lookup(name, strict)", Visibility::Private, Vec::new());
        let delta = inspect(
            &change(
                ChangeKind::SignatureChanged,
                Some(reference("def lookup(name)")),
                Some(reference("def lookup(name, strict)")),
            ),
            Some(&before),
            Some(&after),
            Some(&impact(&["handler"])),
        );

        assert_eq!(delta.change_kind, ChangeKind::SignatureChanged);
        assert_eq!(delta.confidence, Confidence::CERTAIN);
        assert_eq!(delta.before_signature.as_deref(), Some("def lookup(name)"));
        assert_eq!(
            delta.after_signature.as_deref(),
            Some("def lookup(name, strict)")
        );
        assert_eq!(delta.visibility_before.as_deref(), Some("public"));
        assert_eq!(delta.visibility_after.as_deref(), Some("private"));
        assert!(delta.parameters.is_empty());
        assert!(delta.returns.is_empty());
        assert_eq!(delta.affected, vec!["handler".to_owned()]);
        assert!(
            delta
                .evidence
                .iter()
                .any(|line| line.contains("parameters and return type unknown"))
        );
    }

    #[test]
    fn an_added_symbol_has_no_before() {
        let after = symbol("def lookup(name)", Visibility::Public, Vec::new());
        let delta = inspect(
            &change(ChangeKind::Added, None, Some(reference("def lookup(name)"))),
            None,
            Some(&after),
            Some(&impact(&[])),
        );

        assert_eq!(delta.change_kind, ChangeKind::Added);
        assert_eq!(delta.before_signature, None);
        assert_eq!(delta.visibility_before, None);
        assert_eq!(delta.after_signature.as_deref(), Some("def lookup(name)"));
        assert!(delta.affected.is_empty());
        assert!(
            delta
                .evidence
                .iter()
                .any(|line| line.contains("no symbol before"))
        );
        assert!(
            delta
                .evidence
                .iter()
                .any(|line| line.contains("no direct caller"))
        );
    }

    #[test]
    fn missing_impact_is_unknown_not_empty() {
        let after = symbol("def lookup(name)", Visibility::Unknown, Vec::new());
        let delta = inspect(
            &change(
                ChangeKind::Modified,
                None,
                Some(reference("def lookup(name)")),
            ),
            None,
            Some(&after),
            None,
        );

        assert!(delta.affected.is_empty());
        assert_eq!(delta.visibility_after.as_deref(), Some("unknown"));
        assert!(
            delta
                .evidence
                .iter()
                .any(|line| line.contains("affected callers unknown"))
        );
    }

    #[test]
    fn a_risk_marker_becomes_an_effect() {
        let before = symbol("def lookup(name)", Visibility::Public, Vec::new());
        let after = symbol(
            "def lookup(name)",
            Visibility::Public,
            vec![
                marker(MarkerKind::ShellExecution),
                marker(MarkerKind::ShellExecution),
            ],
        );
        let delta = inspect(
            &change(
                ChangeKind::Modified,
                Some(reference("def lookup(name)")),
                Some(reference("def lookup(name)")),
            ),
            Some(&before),
            Some(&after),
            Some(&impact(&["handler"])),
        );

        assert_eq!(delta.effects, vec!["invokes a shell".to_owned()]);
        assert!(
            !delta
                .evidence
                .iter()
                .any(|line| line.contains("parameters and return type unknown"))
        );
    }
}
