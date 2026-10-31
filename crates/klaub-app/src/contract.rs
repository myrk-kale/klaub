//! What changed about one symbol's contract, stated from evidence the analysis already has.
//!
//! A diff shows that lines moved. This module answers the narrower question a reviewer actually has
//! when they open one changed symbol: did its declaration change, what does it promise and what
//! does it touch, and who can reach it now. [`inspect`] is a pure comparison. It reads a
//! [`SymbolChange`], the parsed symbols both sides point at, and the symbol's [`ImpactReport`], and
//! it writes nothing and asks nothing of the file system.
//!
//! Every list is a [`Delta`]: what the symbol had before and what it has after, so a reviewer sees
//! what appeared and what went away. They come from different evidence and are labelled as such:
//!
//! - **signature**: parameters and return type read out of the declaration by [`crate::signature`];
//! - **decorators and endpoints**: the decorators or attributes as written, and the routes or entry
//!   points the parser recognised on the symbol;
//! - **raises**: the exceptions the body names in `raise`/`throw`, plus the error type a Rust
//!   `Result` or a Go `error` return declares. Nothing is inferred from the functions it calls;
//! - **effects**: the risk markers the parser recorded and the calls whose name and receiver look
//!   like network, file or database I/O. This is a judgement from names, not from types;
//! - **members and schema**: for a type, its public members, and what kind of data type it looks
//!   like (a Pydantic model, a dataclass, a serde type, ...).
//!
//! What it deliberately does not do is infer more than that. A fact the inputs cannot answer is
//! said in the evidence as "unknown", never filled with a value that was made up.

use klaub_core::graph::{EntrypointKind, EntrypointScope, ImpactReport};
use klaub_core::ids::UnitId;
use klaub_core::language::Language;
use klaub_core::semantic::{ChangeKind, Confidence, SymbolChange};
use klaub_core::symbol::{MarkerKind, ParsedFile, Symbol, SymbolKind, Visibility};

use crate::signature::{self, SignatureParts};

/// How many direct callers are named. The rest are counted, not listed.
const MAX_AFFECTED: usize = 20;

/// How many reachable entrypoints are named.
const MAX_REACHABLE: usize = 10;

/// A symbol and the parsed file it lives in.
#[derive(Debug, Clone, Copy)]
pub struct SymbolView<'a> {
    /// The parse of the file.
    pub file: &'a ParsedFile,
    /// The symbol's index in that parse.
    pub index: usize,
}

impl<'a> SymbolView<'a> {
    /// The symbol, when the index is valid for the parse.
    pub fn symbol(&self) -> Option<&'a Symbol> {
        self.file.symbols.get(self.index)
    }
}

/// A list of facts before the change and after it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delta {
    /// What the symbol had before. Empty when it did not exist or had none.
    pub before: Vec<String>,
    /// What the symbol has after. Empty when it no longer exists or has none.
    pub after: Vec<String>,
}

impl Delta {
    /// Entries only the new version has.
    pub fn added(&self) -> Vec<&str> {
        self.after
            .iter()
            .filter(|item| !self.before.contains(item))
            .map(String::as_str)
            .collect()
    }

    /// Entries only the old version had.
    pub fn removed(&self) -> Vec<&str> {
        self.before
            .iter()
            .filter(|item| !self.after.contains(item))
            .map(String::as_str)
            .collect()
    }

    /// Entries both versions have.
    pub fn kept(&self) -> Vec<&str> {
        self.after
            .iter()
            .filter(|item| self.before.contains(item))
            .map(String::as_str)
            .collect()
    }

    /// Whether neither version has anything to show.
    pub fn is_empty(&self) -> bool {
        self.before.is_empty() && self.after.is_empty()
    }
}

/// One symbol's contract, compared across the change.
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
    /// The old declaration split into parameters and return type. `None`: not a callable, or a
    /// shape the reader does not understand (the evidence says which).
    pub signature_before: Option<SignatureParts>,
    /// The new declaration split into parameters and return type.
    pub signature_after: Option<SignatureParts>,
    /// Decorators or attributes as written.
    pub decorators: Delta,
    /// Routes and entry points the symbol serves (`GET /login`).
    pub endpoints: Delta,
    /// Exceptions raised in the body and error types the declaration returns.
    pub raises: Delta,
    /// Observable side effects: risk markers and I/O-looking calls.
    pub effects: Delta,
    /// Public members of a type.
    pub members: Delta,
    /// What kind of data type this looks like, when it looks like one.
    pub schema: Option<String>,
    /// Entrypoints that can reach the symbol, shortest path first, capped at 10.
    pub reachable: Vec<String>,
    /// Qualified names of the direct callers, capped at 20.
    pub affected: Vec<String>,
    /// The change's own reasons, plus a note wherever a fact could not be established.
    pub evidence: Vec<String>,
}

/// Compares one symbol's contract.
///
/// `before` and `after` are the parsed symbols the change's references point at. Either may be
/// absent: a symbol that was added has no before, one that was removed has no after, and a
/// reference whose parse is missing has neither. `impact` is absent when the call graph does not
/// know the symbol; that is recorded as unknown rather than as "nothing calls this".
pub fn inspect(
    change: &SymbolChange,
    before: Option<SymbolView<'_>>,
    after: Option<SymbolView<'_>>,
    impact: Option<&ImpactReport>,
) -> ContractDelta {
    let mut evidence = change.evidence.clone();
    let before_symbol = before.and_then(|v| v.symbol());
    let after_symbol = after.and_then(|v| v.symbol());

    let signature_before = split_signature(before);
    let signature_after = split_signature(after);
    note_unsplit(
        before_symbol,
        after_symbol,
        signature_before.as_ref(),
        signature_after.as_ref(),
        &mut evidence,
    );

    let raises = Delta {
        before: raises_of(before),
        after: raises_of(after),
    };
    for item in raises.added() {
        evidence.push(format!("now raises {item}, which it did not before"));
    }
    for item in raises.removed() {
        evidence.push(format!("no longer raises {item}"));
    }

    let affected = affected_callers(impact, &mut evidence);
    if change.before.is_none() {
        evidence.push("no symbol before the change; nothing to compare against".to_owned());
    }
    if change.after.is_none() {
        evidence.push("no symbol after the change; the contract no longer exists".to_owned());
    }
    if change.before.is_some() && before_symbol.is_none()
        || change.after.is_some() && after_symbol.is_none()
    {
        evidence.push(
            "a side of the change has no parse; its decorators, effects and members are unknown"
                .to_owned(),
        );
    }

    ContractDelta {
        unit: change.id.clone(),
        name: change.display_name().to_owned(),
        change_kind: change.kind,
        confidence: change.confidence,
        before_signature: signature_text(
            change.before.as_ref().map(|r| r.signature.as_str()),
            before_symbol,
        ),
        after_signature: signature_text(
            change.after.as_ref().map(|r| r.signature.as_str()),
            after_symbol,
        ),
        visibility_before: before_symbol.map(|s| visibility_label(s.visibility).to_owned()),
        visibility_after: after_symbol.map(|s| visibility_label(s.visibility).to_owned()),
        signature_before,
        signature_after,
        decorators: Delta {
            before: before_symbol
                .map(|s| s.decorators.clone())
                .unwrap_or_default(),
            after: after_symbol
                .map(|s| s.decorators.clone())
                .unwrap_or_default(),
        },
        endpoints: Delta {
            before: endpoints_of(before),
            after: endpoints_of(after),
        },
        raises,
        effects: Delta {
            before: effects_of(before_symbol),
            after: effects_of(after_symbol),
        },
        members: Delta {
            before: members_of(before),
            after: members_of(after),
        },
        schema: after
            .and_then(schema_of)
            .or_else(|| before.and_then(schema_of)),
        reachable: reachable_entrypoints(impact),
        affected,
        evidence,
    }
}

/// The declaration to show: the parsed symbol's when it was reached, otherwise the reference's.
fn signature_text(reference: Option<&str>, symbol: Option<&Symbol>) -> Option<String> {
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

fn split_signature(view: Option<SymbolView<'_>>) -> Option<SignatureParts> {
    let view = view?;
    let symbol = view.symbol()?;
    signature::split(view.file.language, symbol.kind, &symbol.signature)
}

/// Says why the parameters and the return type are missing, when that matters.
///
/// A callable whose declaration changed but could not be read is unknown, and the reviewer is told
/// so. A type has no parameters to split, so nothing is said for it.
fn note_unsplit(
    before: Option<&Symbol>,
    after: Option<&Symbol>,
    parts_before: Option<&SignatureParts>,
    parts_after: Option<&SignatureParts>,
    evidence: &mut Vec<String>,
) {
    let callable = |s: &Symbol| matches!(s.kind, SymbolKind::Function | SymbolKind::Method);
    let unreadable = |symbol: Option<&Symbol>, parts: Option<&SignatureParts>| {
        symbol.is_some_and(callable) && parts.is_none()
    };
    if unreadable(before, parts_before) || unreadable(after, parts_after) {
        evidence.push(
            "parameters and return type unknown: this declaration is not a shape the reader understands"
                .to_owned(),
        );
    }
}

/// The exceptions a symbol can raise, from what is written in its body and in its declaration.
fn raises_of(view: Option<SymbolView<'_>>) -> Vec<String> {
    let Some(view) = view else {
        return Vec::new();
    };
    let Some(symbol) = view.symbol() else {
        return Vec::new();
    };
    let mut raises: Vec<String> = symbol.features.raises.clone();
    if view.file.language == Language::Java {
        // `throws` is part of what a Java method promises its callers, written or not in the body.
        for declared in signature::java_throws(&symbol.signature) {
            let line = format!("declares throws {declared}");
            if !raises.contains(&declared) && !raises.contains(&line) {
                raises.push(line);
            }
        }
    }
    if let Some(parts) = signature::split(view.file.language, symbol.kind, &symbol.signature)
        && let Some(returns) = parts.returns.as_deref()
        && let Some(declared) = declared_error(view.file.language, returns)
        && !raises.contains(&declared)
    {
        raises.push(declared);
    }
    raises
}

/// The error a return type declares: a Rust `Result<_, E>` or a Go `error`.
fn declared_error(language: Language, returns: &str) -> Option<String> {
    match language {
        Language::Rust => {
            let inner = returns.trim().strip_prefix("Result<")?.strip_suffix('>')?;
            let mut depth = 0_i32;
            let mut last_comma = None;
            for (index, c) in inner.char_indices() {
                match c {
                    '<' | '(' | '[' => depth += 1,
                    '>' | ')' | ']' => depth -= 1,
                    ',' if depth == 0 => last_comma = Some(index),
                    _ => {}
                }
            }
            let error = last_comma.map_or("", |at| &inner[at + 1..]).trim();
            Some(if error.is_empty() {
                "returns an error (Result)".to_owned()
            } else {
                format!("returns Err({error})")
            })
        }
        Language::Go => returns
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| word == "error")
            .then(|| "returns an error".to_owned()),
        _ => None,
    }
}

/// Routes and entry points the parser recognised on the symbol.
fn endpoints_of(view: Option<SymbolView<'_>>) -> Vec<String> {
    let Some(view) = view else {
        return Vec::new();
    };
    let mut lines: Vec<String> = view
        .file
        .entrypoints
        .iter()
        .filter(|hint| hint.symbol == Some(view.index))
        .map(|hint| match &hint.kind {
            EntrypointKind::Http { method, route } => format!("{method} {route}"),
            EntrypointKind::Cli => "command line entry".to_owned(),
            EntrypointKind::Main => "program main".to_owned(),
            EntrypointKind::Worker => "background worker".to_owned(),
            EntrypointKind::PublicExport => "public export".to_owned(),
        })
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

/// Risk markers and I/O-looking calls a symbol carries, named once, in a stable order.
fn effects_of(symbol: Option<&Symbol>) -> Vec<String> {
    let Some(symbol) = symbol else {
        return Vec::new();
    };
    let mut kinds: Vec<MarkerKind> = Vec::new();
    for marker in &symbol.features.markers {
        if !kinds.contains(&marker.kind) {
            kinds.push(marker.kind);
        }
    }
    kinds.sort_unstable();
    let mut lines: Vec<String> = kinds
        .iter()
        .map(|kind| effect_line(*kind).to_owned())
        .collect();
    let mut calls: Vec<String> = symbol
        .features
        .io_calls
        .iter()
        .map(|call| format!("calls {call} (looks like I/O)"))
        .collect();
    calls.sort();
    lines.extend(calls);
    lines
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

/// The public members of a type, by name. A function has none.
fn members_of(view: Option<SymbolView<'_>>) -> Vec<String> {
    let Some(view) = view else {
        return Vec::new();
    };
    let Some(symbol) = view.symbol() else {
        return Vec::new();
    };
    let holds_members = matches!(
        symbol.kind,
        SymbolKind::Class
            | SymbolKind::Struct
            | SymbolKind::Interface
            | SymbolKind::Enum
            | SymbolKind::Trait
            | SymbolKind::Impl
    );
    if !holds_members {
        return Vec::new();
    }
    let mut names: Vec<String> = view
        .file
        .children_of(view.index)
        .filter_map(|i| view.file.symbols.get(i))
        .filter(|child| child.visibility != Visibility::Private)
        .map(|child| format!("{} {}", child.kind.label(), child.name))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// What kind of data type a symbol looks like, from its declaration and its decorators.
fn schema_of(view: SymbolView<'_>) -> Option<String> {
    let symbol = view.symbol()?;
    let signature = symbol.signature.as_str();
    let has_decorator = |needle: &str| symbol.decorators.iter().any(|d| d.contains(needle));
    let label = match view.file.language {
        Language::Python if symbol.kind == SymbolKind::Class => {
            if signature.contains("BaseModel") {
                "Pydantic model"
            } else if signature.contains("TypedDict") {
                "TypedDict"
            } else if signature.contains("NamedTuple") {
                "NamedTuple"
            } else if signature.contains("Enum") {
                "enum"
            } else if has_decorator("dataclass") {
                "dataclass"
            } else {
                return None;
            }
        }
        Language::TypeScript | Language::Tsx | Language::JavaScript => match symbol.kind {
            SymbolKind::Interface => "interface",
            SymbolKind::TypeAlias => "type alias",
            SymbolKind::Enum => "enum",
            _ => return None,
        },
        Language::Rust
            if matches!(symbol.kind, SymbolKind::Struct | SymbolKind::Enum)
                && (has_decorator("Serialize") || has_decorator("Deserialize")) =>
        {
            "serde type"
        }
        Language::Rust if symbol.kind == SymbolKind::Trait => "trait",
        Language::Go if symbol.kind == SymbolKind::Struct => "struct",
        Language::Go if symbol.kind == SymbolKind::Interface => "interface",
        Language::Java => match symbol.kind {
            SymbolKind::Struct => "record",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            _ => return None,
        },
        _ => return None,
    };
    Some(label.to_owned())
}

/// The entrypoints that reach the symbol, named by what they are and how sure the path is.
fn reachable_entrypoints(impact: Option<&ImpactReport>) -> Vec<String> {
    let Some(impact) = impact else {
        return Vec::new();
    };
    impact
        .entrypoints
        .iter()
        .take(MAX_REACHABLE)
        .map(|reach| {
            let what = match &reach.entrypoint.kind {
                EntrypointKind::Http { method, route } => format!("{method} {route}"),
                EntrypointKind::Cli => "command line entry".to_owned(),
                EntrypointKind::Main => "program main".to_owned(),
                EntrypointKind::Worker => "background worker".to_owned(),
                EntrypointKind::PublicExport => "public export".to_owned(),
            };
            let scope = match reach.entrypoint.scope {
                EntrypointScope::External => "external",
                EntrypointScope::Internal => "internal",
            };
            format!(
                "{what} ({scope}, {}, {} hop(s))",
                reach.origin.label(),
                reach.path.len().saturating_sub(1)
            )
        })
        .collect()
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
        CallerLevel, EdgeOrigin, Entrypoint, EntrypointReach, ImpactCoverage, ImpactScope,
        PathStep, SymbolKey,
    };
    use klaub_core::path::RepoPath;
    use klaub_core::semantic::{SymbolRef, symbol_unit_id};
    use klaub_core::symbol::{EntrypointHint, Fingerprint, Marker, Span, SymbolFeatures};

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

    fn symbol(
        name: &str,
        kind: SymbolKind,
        signature: &str,
        visibility: Visibility,
        parent: Option<usize>,
    ) -> Symbol {
        Symbol {
            name: name.to_owned(),
            qualified_name: name.to_owned(),
            kind,
            signature: signature.to_owned(),
            visibility,
            span: span(),
            parent,
            decorators: Vec::new(),
            fingerprint: Fingerprint::default(),
            features: SymbolFeatures::default(),
        }
    }

    fn function(signature: &str, visibility: Visibility) -> Symbol {
        symbol(
            "lookup",
            SymbolKind::Function,
            signature,
            visibility,
            Some(0),
        )
    }

    fn file(language: Language, symbols: Vec<Symbol>) -> ParsedFile {
        let mut all = vec![symbol(
            "<module>",
            SymbolKind::Module,
            "",
            Visibility::Public,
            None,
        )];
        all.extend(symbols);
        ParsedFile {
            language,
            symbols: all,
            imports: Vec::new(),
            calls: Vec::new(),
            entrypoints: Vec::new(),
            has_syntax_errors: false,
            parser_version: 2,
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn view(file: &ParsedFile) -> Option<SymbolView<'_>> {
        Some(SymbolView { file, index: 1 })
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

    fn modified(before: &str, after: &str) -> SymbolChange {
        change(
            ChangeKind::Modified,
            Some(reference(before)),
            Some(reference(after)),
        )
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
    fn a_changed_signature_is_split_into_parameters_and_return() {
        let before = file(
            Language::Python,
            vec![function("def lookup(name) -> str", Visibility::Public)],
        );
        let after = file(
            Language::Python,
            vec![function(
                "def lookup(name, strict: bool = False) -> Optional[str]",
                Visibility::Private,
            )],
        );
        let delta = inspect(
            &modified(
                "def lookup(name) -> str",
                "def lookup(name, strict: bool = False) -> Optional[str]",
            ),
            view(&before),
            view(&after),
            Some(&impact(&["handler"])),
        );

        assert_eq!(delta.confidence, Confidence::CERTAIN);
        assert_eq!(delta.visibility_before.as_deref(), Some("public"));
        assert_eq!(delta.visibility_after.as_deref(), Some("private"));
        assert_eq!(
            delta.signature_before,
            Some(SignatureParts {
                parameters: vec!["name".to_owned()],
                returns: Some("str".to_owned())
            })
        );
        let after_parts = delta.signature_after.unwrap();
        assert_eq!(after_parts.parameters, vec!["name", "strict: bool = False"]);
        assert_eq!(after_parts.returns.as_deref(), Some("Optional[str]"));
        assert_eq!(delta.affected, vec!["handler".to_owned()]);
    }

    #[test]
    fn an_unreadable_declaration_is_reported_unknown() {
        let after = file(
            Language::Python,
            vec![function("lambda x: x", Visibility::Public)],
        );
        let delta = inspect(
            &change(ChangeKind::Added, None, Some(reference("lambda x: x"))),
            None,
            view(&after),
            Some(&impact(&[])),
        );
        assert!(delta.signature_after.is_none());
        assert!(
            delta
                .evidence
                .iter()
                .any(|line| line.contains("parameters and return type unknown"))
        );
    }

    #[test]
    fn an_added_symbol_has_no_before() {
        let after = file(
            Language::Python,
            vec![function("def lookup(name)", Visibility::Public)],
        );
        let delta = inspect(
            &change(ChangeKind::Added, None, Some(reference("def lookup(name)"))),
            None,
            view(&after),
            Some(&impact(&[])),
        );

        assert_eq!(delta.before_signature, None);
        assert_eq!(delta.visibility_before, None);
        assert_eq!(delta.after_signature.as_deref(), Some("def lookup(name)"));
        assert!(delta.affected.is_empty());
        assert!(
            delta
                .evidence
                .iter()
                .any(|l| l.contains("no symbol before"))
        );
        assert!(
            delta
                .evidence
                .iter()
                .any(|l| l.contains("no direct caller"))
        );
    }

    #[test]
    fn missing_impact_is_unknown_not_empty() {
        let after = file(
            Language::Python,
            vec![function("def lookup(name)", Visibility::Unknown)],
        );
        let delta = inspect(
            &change(
                ChangeKind::Modified,
                None,
                Some(reference("def lookup(name)")),
            ),
            None,
            view(&after),
            None,
        );
        assert!(delta.affected.is_empty());
        assert_eq!(delta.visibility_after.as_deref(), Some("unknown"));
        assert!(
            delta
                .evidence
                .iter()
                .any(|l| l.contains("affected callers unknown"))
        );
    }

    #[test]
    fn effects_show_what_appeared_and_what_went_away() {
        let mut old = function("def lookup(name)", Visibility::Public);
        old.features.markers = vec![marker(MarkerKind::SwallowedError)];
        let mut new = function("def lookup(name)", Visibility::Public);
        new.features.markers = vec![
            marker(MarkerKind::ShellExecution),
            marker(MarkerKind::ShellExecution),
        ];
        new.features.io_calls = vec!["requests.get".to_owned()];
        let before = file(Language::Python, vec![old]);
        let after = file(Language::Python, vec![new]);
        let delta = inspect(
            &modified("def lookup(name)", "def lookup(name)"),
            view(&before),
            view(&after),
            Some(&impact(&["handler"])),
        );

        assert_eq!(
            delta.effects.added(),
            vec!["invokes a shell", "calls requests.get (looks like I/O)"]
        );
        assert_eq!(delta.effects.removed(), vec!["swallows an error"]);
        assert!(
            !delta
                .evidence
                .iter()
                .any(|l| l.contains("parameters and return type unknown"))
        );
    }

    #[test]
    fn a_new_raise_is_a_new_failure_path_for_callers() {
        let mut old = function("def lookup(name)", Visibility::Public);
        old.features.raises = vec!["KeyError".to_owned()];
        let mut new = function("def lookup(name)", Visibility::Public);
        new.features.raises = vec!["KeyError".to_owned(), "ValueError".to_owned()];
        let before = file(Language::Python, vec![old]);
        let after = file(Language::Python, vec![new]);
        let delta = inspect(
            &modified("def lookup(name)", "def lookup(name)"),
            view(&before),
            view(&after),
            None,
        );
        assert_eq!(delta.raises.added(), vec!["ValueError"]);
        assert_eq!(delta.raises.kept(), vec!["KeyError"]);
        assert!(
            delta
                .evidence
                .iter()
                .any(|l| l == "now raises ValueError, which it did not before")
        );
    }

    #[test]
    fn a_rust_result_and_a_go_error_declare_what_can_fail() {
        let rust = file(
            Language::Rust,
            vec![function(
                "pub fn load(path: &str) -> Result<Config, LoadError>",
                Visibility::Public,
            )],
        );
        let delta = inspect(
            &change(
                ChangeKind::Added,
                None,
                Some(reference(
                    "pub fn load(path: &str) -> Result<Config, LoadError>",
                )),
            ),
            None,
            view(&rust),
            None,
        );
        assert_eq!(delta.raises.after, vec!["returns Err(LoadError)"]);

        let go = file(
            Language::Go,
            vec![function(
                "func Load(p string) (Config, error)",
                Visibility::Public,
            )],
        );
        let delta = inspect(
            &change(
                ChangeKind::Added,
                None,
                Some(reference("func Load(p string) (Config, error)")),
            ),
            None,
            view(&go),
            None,
        );
        assert_eq!(delta.raises.after, vec!["returns an error"]);
    }

    #[test]
    fn decorators_and_endpoints_are_compared() {
        let mut old = function("def lookup(name)", Visibility::Public);
        old.decorators = vec!["app.get(\"/a\")".to_owned()];
        let mut new = function("def lookup(name)", Visibility::Public);
        new.decorators = vec!["app.get(\"/b\")".to_owned(), "cache".to_owned()];
        let mut before = file(Language::Python, vec![old]);
        let mut after = file(Language::Python, vec![new]);
        let hint = |route: &str| EntrypointHint {
            symbol: Some(1),
            scope: EntrypointScope::External,
            kind: EntrypointKind::Http {
                method: "GET".to_owned(),
                route: route.to_owned(),
            },
            origin: EdgeOrigin::Resolved,
            evidence: "decorator".to_owned(),
            line: 1,
        };
        before.entrypoints = vec![hint("/a")];
        after.entrypoints = vec![hint("/b")];
        let delta = inspect(
            &modified("def lookup(name)", "def lookup(name)"),
            view(&before),
            view(&after),
            None,
        );
        assert_eq!(delta.endpoints.removed(), vec!["GET /a"]);
        assert_eq!(delta.endpoints.added(), vec!["GET /b"]);
        assert_eq!(delta.decorators.added(), vec!["app.get(\"/b\")", "cache"]);
        assert_eq!(delta.decorators.removed(), vec!["app.get(\"/a\")"]);
    }

    #[test]
    fn a_type_reports_its_public_members_and_what_kind_of_data_it_is() {
        let class = |members: &[&str]| {
            let mut symbols = vec![symbol(
                "User",
                SymbolKind::Class,
                "class User(BaseModel)",
                Visibility::Public,
                Some(0),
            )];
            for name in members {
                symbols.push(symbol(
                    name,
                    SymbolKind::Method,
                    &format!("def {name}(self)"),
                    Visibility::Public,
                    Some(1),
                ));
            }
            symbols.push(symbol(
                "_hidden",
                SymbolKind::Method,
                "def _hidden(self)",
                Visibility::Private,
                Some(1),
            ));
            file(Language::Python, symbols)
        };
        let before = class(&["save"]);
        let after = class(&["save", "email"]);
        let user_ref = SymbolRef {
            kind: SymbolKind::Class,
            qualified_name: "User".to_owned(),
            signature: "class User(BaseModel)".to_owned(),
            ..reference("")
        };
        let delta = inspect(
            &change(ChangeKind::Modified, Some(user_ref.clone()), Some(user_ref)),
            view(&before),
            view(&after),
            None,
        );
        assert_eq!(delta.schema.as_deref(), Some("Pydantic model"));
        assert_eq!(delta.members.added(), vec!["method email"]);
        assert!(delta.members.removed().is_empty());
        assert!(
            delta.members.after.iter().all(|m| !m.contains("_hidden")),
            "private members are not part of the contract"
        );
    }

    #[test]
    fn entrypoints_that_reach_the_symbol_are_named_with_their_certainty() {
        let after = file(
            Language::Python,
            vec![function("def lookup(name)", Visibility::Public)],
        );
        let mut report = impact(&["handler"]);
        report.entrypoints = vec![EntrypointReach {
            entrypoint: Entrypoint {
                symbol: SymbolKey::new(path(), "handler"),
                scope: EntrypointScope::External,
                kind: EntrypointKind::Http {
                    method: "POST".to_owned(),
                    route: "/login".to_owned(),
                },
                origin: EdgeOrigin::Resolved,
                evidence: "decorator".to_owned(),
            },
            path: vec![
                PathStep {
                    symbol: SymbolKey::new(path(), "handler"),
                    origin: EdgeOrigin::Resolved,
                },
                PathStep {
                    symbol: SymbolKey::new(path(), "lookup"),
                    origin: EdgeOrigin::Heuristic,
                },
            ],
            origin: EdgeOrigin::Heuristic,
        }];
        let delta = inspect(
            &change(
                ChangeKind::Modified,
                None,
                Some(reference("def lookup(name)")),
            ),
            None,
            view(&after),
            Some(&report),
        );
        assert_eq!(delta.reachable.len(), 1);
        assert!(delta.reachable[0].starts_with("POST /login (external, heuristic"));
        assert!(delta.reachable[0].ends_with("1 hop(s))"));
    }
}
