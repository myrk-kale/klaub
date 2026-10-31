//! # klaub-flow
//!
//! A pure domain service for flow documents written by an agent through the `klaub-flow` skill.
//!
//! The document is **untrusted** (§11.6, §20): it is parsed with a strict schema and bounded sizes,
//! rejected when it contains terminal sequences, links outside the worktree or references to files
//! that are not part of the project, and judged `current`, `stale`, `invalid` or `missing` against
//! the snapshot under review. What the author claims is a *hypothesis of navigation*: where the
//! index disagrees (a "resolved" step whose symbol does not exist) the difference is shown.

use std::collections::{HashMap, HashSet};

use klaub_core::flow::{FLOW_SCHEMA_VERSION, FlowDocument, FlowState};
use klaub_core::graph::{EdgeKind, EdgeOrigin, SymbolKey};
use klaub_core::ids::{SnapshotId, WorktreeId};
use klaub_core::path::RepoPath;

const MAX_STEPS: usize = 500;
const MAX_EDGES: usize = 2000;
const MAX_ID_CHARS: usize = 64;

/// What validation needs to know about the snapshot and the project.
pub struct FlowContext<'a> {
    /// The snapshot under review.
    pub snapshot_id: &'a SnapshotId,
    /// Its worktree.
    pub worktree_id: &'a WorktreeId,
    /// Its base commit.
    pub base_oid: &'a str,
    /// Largest accepted document.
    pub max_bytes: u64,
    /// Whether a path is a file of the project (changed or not).
    pub knows_file: &'a dyn Fn(&RepoPath) -> bool,
    /// Whether the file defines a symbol with this qualified name.
    pub symbol_exists: &'a dyn Fn(&RepoPath, &str) -> bool,
    /// Whether the call graph connects two symbols: `Some(false)` is a disagreement, `None` unknown.
    pub connected: &'a dyn Fn(&SymbolKey, &SymbolKey) -> Option<bool>,
}

/// A step after checking it against the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStep {
    /// The step id.
    pub id: String,
    /// Its file, when it points into the project.
    pub file: Option<RepoPath>,
    /// The symbol as written.
    pub symbol: Option<String>,
    /// The symbol exists in the file according to the parser's index.
    pub symbol_found: bool,
}

/// A document that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFlow {
    /// The parsed document.
    pub document: FlowDocument,
    /// Its steps resolved against the index (same order).
    pub steps: Vec<ResolvedStep>,
    /// Places where the author and the index disagree.
    pub divergences: Vec<String>,
}

/// The outcome of validating a flow file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    /// Valid and for this snapshot.
    Current(Box<ValidatedFlow>),
    /// Valid but written for another snapshot; it may be consulted with a warning.
    Stale {
        /// The snapshot id the document claims.
        found: String,
        /// The document, structurally valid.
        flow: Box<ValidatedFlow>,
    },
    /// Rejected, with every problem found.
    Invalid(Vec<String>),
}

impl Validation {
    /// The state label used by the CLI and the exports.
    pub fn state(&self) -> FlowState {
        match self {
            Self::Current(_) => FlowState::Current,
            Self::Stale { found, .. } => FlowState::Stale {
                found: found.clone(),
            },
            Self::Invalid(issues) => FlowState::Invalid {
                reason: issues.first().cloned().unwrap_or_default(),
            },
        }
    }
}

fn bad_text(text: &str, allow_newlines: bool) -> bool {
    text.chars()
        .any(|c| c.is_control() && !(allow_newlines && matches!(c, '\n' | '\t')))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_ID_CHARS
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

/// Validates the bytes of a flow file against the snapshot.
pub fn validate(bytes: &[u8], context: &FlowContext<'_>) -> Validation {
    if bytes.len() as u64 > context.max_bytes {
        return Validation::Invalid(vec![format!(
            "the file is larger than the {} byte limit",
            context.max_bytes
        )]);
    }
    let document: FlowDocument = match serde_json::from_slice(bytes) {
        Ok(document) => document,
        Err(error) => {
            return Validation::Invalid(vec![format!("not a valid flow document: {error}")]);
        }
    };
    let mut issues = Vec::new();
    if document.schema_version != FLOW_SCHEMA_VERSION {
        issues.push(format!(
            "unsupported schema_version {} (expected {FLOW_SCHEMA_VERSION})",
            document.schema_version
        ));
    }
    if document.steps.is_empty() {
        issues.push("the document has no steps".to_owned());
    }
    if document.steps.len() > MAX_STEPS {
        issues.push(format!("more than {MAX_STEPS} steps"));
    }
    if document.edges.len() > MAX_EDGES {
        issues.push(format!("more than {MAX_EDGES} edges"));
    }
    for (name, text, newlines, limit) in [
        ("title", &document.title, false, 200),
        ("summary", &document.summary, true, 2000),
        ("generated_by", &document.generated_by, false, 100),
    ] {
        if bad_text(text, newlines) {
            issues.push(format!("{name} contains control or terminal characters"));
        }
        if text.chars().count() > limit {
            issues.push(format!("{name} is longer than {limit} characters"));
        }
    }

    let mut seen = HashSet::new();
    let mut steps = Vec::with_capacity(document.steps.len());
    let mut divergences = Vec::new();
    for step in &document.steps {
        if !valid_id(&step.id) {
            issues.push(format!(
                "step id {:?} is empty, too long or has unsafe characters",
                step.id
            ));
        } else if !seen.insert(step.id.as_str()) {
            issues.push(format!("step id {:?} is used twice", step.id));
        }
        if bad_text(&step.note, true) || step.note.chars().count() > 1000 {
            issues.push(format!(
                "step {:?}: the note contains control characters or is too long",
                step.id
            ));
        }
        let mut file = None;
        let mut symbol_found = false;
        match (&step.file_path, &step.external_ref) {
            (Some(raw), _) => {
                match RepoPath::new(raw) {
                    Err(_) => issues.push(format!(
                        "step {:?}: {raw:?} is not a relative path inside the worktree",
                        step.id
                    )),
                    Ok(path) if !(context.knows_file)(&path) => {
                        issues.push(format!(
                            "step {:?}: {path} is not a file of this project",
                            step.id
                        ));
                    }
                    Ok(path) => {
                        if let Some(symbol) = &step.symbol {
                            if bad_text(symbol, false) {
                                issues.push(format!(
                                    "step {:?}: the symbol name contains control characters",
                                    step.id
                                ));
                            } else {
                                symbol_found = (context.symbol_exists)(&path, symbol);
                                if !symbol_found && step.confidence == EdgeOrigin::Resolved {
                                    divergences.push(format!("step {:?} claims `resolved` but {path} defines no `{symbol}`", step.id));
                                }
                            }
                        }
                        file = Some(path);
                    }
                }
            }
            (None, Some(external)) => {
                if bad_text(&external.kind, false) || bad_text(&external.name, false) {
                    issues.push(format!(
                        "step {:?}: the external reference contains control characters",
                        step.id
                    ));
                }
            }
            (None, None) => issues.push(format!(
                "step {:?} has neither a file_path nor an external_ref",
                step.id
            )),
        }
        steps.push(ResolvedStep {
            id: step.id.clone(),
            file,
            symbol: step.symbol.clone(),
            symbol_found,
        });
    }

    let index_of: HashMap<&str, usize> = document
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    for edge in &document.edges {
        let (Some(&from), Some(&to)) = (
            index_of.get(edge.from.as_str()),
            index_of.get(edge.to.as_str()),
        ) else {
            issues.push(format!(
                "edge {:?} -> {:?} refers to an unknown step",
                edge.from, edge.to
            ));
            continue;
        };
        if edge.kind == EdgeKind::Calls
            && let Some((a, b)) = (&steps[from], &steps[to]).pair()
            && (context.connected)(&a, &b) == Some(false)
        {
            divergences.push(format!(
                "edge {} -> {} is described as a call but the call graph has no such call",
                edge.from, edge.to
            ));
        }
    }

    if !issues.is_empty() {
        return Validation::Invalid(issues);
    }
    let flow = ValidatedFlow {
        document,
        steps,
        divergences,
    };
    if &flow.document.snapshot_id != context.snapshot_id {
        return Validation::Stale {
            found: flow.document.snapshot_id.to_string(),
            flow: Box::new(flow),
        };
    }
    if &flow.document.worktree_id != context.worktree_id
        || flow.document.base_oid != context.base_oid
    {
        return Validation::Invalid(vec![
            "the document belongs to another worktree or base commit".to_owned(),
        ]);
    }
    Validation::Current(Box::new(flow))
}

/// Helper to get both symbol keys of an edge.
trait StepPair {
    fn pair(&self) -> Option<(SymbolKey, SymbolKey)>;
}

impl StepPair for (&ResolvedStep, &ResolvedStep) {
    fn pair(&self) -> Option<(SymbolKey, SymbolKey)> {
        let key = |s: &ResolvedStep| {
            if !s.symbol_found {
                return None;
            }
            Some(SymbolKey::new(s.file.clone()?, s.symbol.clone()?))
        };
        Some((key(self.0)?, key(self.1)?))
    }
}

/// The instruction that asks the agent to produce the flow (one line, never submitted by Klaub).
pub fn request_prompt(snapshot_id: &SnapshotId, output_path: &str) -> String {
    format!(
        "Use the klaub-flow skill to explain the execution flow of the change in snapshot {snapshot_id}. \
Consult `klaub flow context --snapshot {snapshot_id} --format json`, point to real files and symbols, \
state your uncertainty, and save the result to {output_path} (schema_version {FLOW_SCHEMA_VERSION}). \
Validate it with `klaub flow validate {output_path}`. Do not change any source code."
    )
}

/// One row of the flow as the step list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowRow {
    /// Index into the document's steps.
    pub step: usize,
    /// Nesting depth from the first step.
    pub depth: usize,
    /// The step was already shown above (a cycle or a join): not expanded again.
    pub revisit: bool,
    /// The relation from the parent row.
    pub via: Option<(EdgeKind, EdgeOrigin)>,
}

/// Lays the flow out as an indented list: depth-first from the first step, branches in document
/// order, cycles marked instead of followed, unreachable steps appended at depth 0.
pub fn layout(document: &FlowDocument) -> Vec<FlowRow> {
    let index_of: HashMap<&str, usize> = document
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut children: Vec<Vec<(usize, EdgeKind, EdgeOrigin)>> =
        vec![Vec::new(); document.steps.len()];
    for edge in &document.edges {
        if let (Some(&from), Some(&to)) = (
            index_of.get(edge.from.as_str()),
            index_of.get(edge.to.as_str()),
        ) {
            children[from].push((to, edge.kind, edge.confidence));
        }
    }
    let mut rows = Vec::with_capacity(document.steps.len());
    let mut shown = vec![false; document.steps.len()];
    let mut roots: Vec<usize> = Vec::new();
    if !document.steps.is_empty() {
        roots.push(0);
    }
    roots.extend(1..document.steps.len());
    for root in roots {
        if shown[root] {
            continue;
        }
        let mut stack = vec![(root, 0_usize, None)];
        while let Some((step, depth, via)) = stack.pop() {
            let revisit = shown[step];
            rows.push(FlowRow {
                step,
                depth,
                revisit,
                via,
            });
            if revisit {
                continue;
            }
            shown[step] = true;
            for &(child, kind, origin) in children[step].iter().rev() {
                stack.push((child, depth + 1, Some((kind, origin))));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SNAP: &str = "snap_abc123";

    fn context<'a>(
        snapshot: &'a SnapshotId,
        worktree: &'a WorktreeId,
        known: &'a dyn Fn(&RepoPath) -> bool,
        symbols: &'a dyn Fn(&RepoPath, &str) -> bool,
        connected: &'a dyn Fn(&SymbolKey, &SymbolKey) -> Option<bool>,
    ) -> FlowContext<'a> {
        FlowContext {
            snapshot_id: snapshot,
            worktree_id: worktree,
            base_oid: "oid_da_base",
            max_bytes: 1 << 20,
            knows_file: known,
            symbol_exists: symbols,
            connected,
        }
    }

    fn document() -> serde_json::Value {
        json!({
            "schema_version": 1, "snapshot_id": SNAP, "base_oid": "oid_da_base", "worktree_id": "wt_123",
            "title": "Authentication flow", "summary": "How a request reaches validation", "generated_by": "codex",
            "steps": [
                {"id": "request", "file_path": "api/routes/auth.py", "symbol": "login", "note": "Receives the login", "changed": false, "confidence": "resolved"},
                {"id": "validation", "file_path": "domain/auth.py", "symbol": "validate_credentials", "note": "Validates", "changed": true, "confidence": "heuristic"},
                {"id": "store", "external_ref": {"kind": "database", "name": "users table"}, "note": "reads", "confidence": "unresolved"}
            ],
            "edges": [
                {"from": "request", "to": "validation", "kind": "calls", "confidence": "heuristic"},
                {"from": "validation", "to": "store", "kind": "references", "confidence": "unresolved"}
            ]
        })
    }

    fn run(doc: &serde_json::Value) -> Validation {
        let snapshot = SnapshotId::parse(SNAP).unwrap();
        let worktree = WorktreeId::parse("wt_123").unwrap();
        let known = |p: &RepoPath| matches!(p.as_str(), "api/routes/auth.py" | "domain/auth.py");
        let symbols = |p: &RepoPath, s: &str| {
            (p.as_str(), s) == ("api/routes/auth.py", "login")
                || (p.as_str(), s) == ("domain/auth.py", "validate_credentials")
        };
        let connected = |_: &SymbolKey, _: &SymbolKey| Some(true);
        validate(
            doc.to_string().as_bytes(),
            &context(&snapshot, &worktree, &known, &symbols, &connected),
        )
    }

    fn invalid(doc: &serde_json::Value) -> Vec<String> {
        match run(doc) {
            Validation::Invalid(issues) => issues,
            other => panic!("expected invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_good_document_is_current() {
        let Validation::Current(flow) = run(&document()) else {
            panic!("expected current")
        };
        assert_eq!(flow.steps.len(), 3);
        assert!(flow.steps[0].symbol_found && flow.steps[1].symbol_found);
        assert_eq!(flow.steps[2].file, None);
        assert!(flow.divergences.is_empty());
        assert_eq!(run(&document()).state(), FlowState::Current);
    }

    #[test]
    fn another_snapshot_is_stale_but_still_inspectable() {
        let mut doc = document();
        doc["snapshot_id"] = json!("snap_old999");
        let Validation::Stale { found, flow } = run(&doc) else {
            panic!("expected stale")
        };
        assert_eq!(found, "snap_old999");
        assert_eq!(flow.document.steps.len(), 3);
        assert_eq!(run(&doc).state().label(), "stale");
    }

    #[test]
    fn hostile_documents_are_rejected_with_every_reason() {
        let mut doc = document();
        doc["steps"][0]["file_path"] = json!("../../etc/passwd");
        doc["steps"][1]["note"] = json!("click \u{1b}]8;;http://evil\u{7}here");
        doc["steps"][2]["id"] = json!("request");
        doc["edges"][1]["to"] = json!("nowhere");
        doc["title"] = json!("bad \u{1b}[31mtitle");
        let issues = invalid(&doc);
        for expected in [
            "not a relative path",
            "control characters",
            "used twice",
            "unknown step",
            "title contains",
        ] {
            assert!(
                issues.iter().any(|i| i.contains(expected)),
                "missing {expected:?} in {issues:#?}"
            );
        }
    }

    #[test]
    fn files_outside_the_project_and_wrong_schemas_are_rejected() {
        let mut doc = document();
        doc["steps"][0]["file_path"] = json!("api/routes/secret.py");
        assert!(
            invalid(&doc)
                .iter()
                .any(|i| i.contains("not a file of this project"))
        );
        let mut doc = document();
        doc["schema_version"] = json!(2);
        assert!(invalid(&doc).iter().any(|i| i.contains("schema_version")));
        assert!(invalid(&json!({"hello": "world"}))[0].starts_with("not a valid flow document"));
        let snapshot = SnapshotId::parse(SNAP).unwrap();
        let worktree = WorktreeId::parse("wt_123").unwrap();
        let (k, s, c) = (
            |_: &RepoPath| true,
            |_: &RepoPath, _: &str| true,
            |_: &SymbolKey, _: &SymbolKey| None,
        );
        let mut small = context(&snapshot, &worktree, &k, &s, &c);
        small.max_bytes = 10;
        assert!(matches!(
            validate(document().to_string().as_bytes(), &small),
            Validation::Invalid(_)
        ));
    }

    #[test]
    fn claims_the_index_contradicts_become_divergences() {
        let mut doc = document();
        doc["steps"][1]["symbol"] = json!("does_not_exist");
        doc["steps"][1]["confidence"] = json!("resolved");
        let Validation::Current(flow) = run(&doc) else {
            panic!("still structurally valid")
        };
        assert!(!flow.steps[1].symbol_found);
        assert!(
            flow.divergences
                .iter()
                .any(|d| d.contains("claims `resolved`")),
            "{:?}",
            flow.divergences
        );
    }

    #[test]
    fn a_call_the_graph_does_not_know_is_flagged() {
        let snapshot = SnapshotId::parse(SNAP).unwrap();
        let worktree = WorktreeId::parse("wt_123").unwrap();
        let known = |_: &RepoPath| true;
        let symbols = |_: &RepoPath, _: &str| true;
        let connected = |_: &SymbolKey, _: &SymbolKey| Some(false);
        let Validation::Current(flow) = validate(
            document().to_string().as_bytes(),
            &context(&snapshot, &worktree, &known, &symbols, &connected),
        ) else {
            panic!("expected current")
        };
        assert!(flow.divergences.iter().any(|d| d.contains("no such call")));
    }

    #[test]
    fn the_request_prompt_is_one_line_with_the_real_ids() {
        let prompt = request_prompt(
            &SnapshotId::parse(SNAP).unwrap(),
            ".klaub/flows/snap_abc123.json",
        );
        assert!(!prompt.contains('\n'));
        assert!(
            prompt.contains("klaub-flow skill")
                && prompt.contains("snap_abc123")
                && prompt.contains("Do not change any source code")
        );
    }

    #[test]
    fn layout_indents_branches_and_marks_cycles() {
        let doc: FlowDocument = serde_json::from_value(json!({
            "schema_version": 1, "snapshot_id": SNAP, "base_oid": "o", "worktree_id": "wt_1", "title": "t",
            "steps": [
                {"id": "a", "file_path": "a.py", "confidence": "resolved"},
                {"id": "b", "file_path": "b.py", "confidence": "resolved"},
                {"id": "c", "file_path": "c.py", "confidence": "heuristic"},
                {"id": "orphan", "file_path": "o.py", "confidence": "unresolved"}
            ],
            "edges": [
                {"from": "a", "to": "b", "kind": "calls", "confidence": "resolved"},
                {"from": "a", "to": "c", "kind": "calls", "confidence": "heuristic"},
                {"from": "b", "to": "a", "kind": "calls", "confidence": "resolved"}
            ]
        }))
        .unwrap();
        let rows = layout(&doc);
        let shape: Vec<(usize, usize, bool)> =
            rows.iter().map(|r| (r.step, r.depth, r.revisit)).collect();
        assert_eq!(
            shape,
            vec![
                (0, 0, false),
                (1, 1, false),
                (0, 2, true),
                (2, 1, false),
                (3, 0, false)
            ]
        );
        assert_eq!(rows[3].via, Some((EdgeKind::Calls, EdgeOrigin::Heuristic)));
    }
}
