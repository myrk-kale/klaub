//! End-to-end matching with the real parser: realistic edits and the classification they get.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_raw_string_hashes,
    clippy::format_collect
)]

use klaub_core::language::Language;
use klaub_core::path::RepoPath;
use klaub_core::ports::SourceParser;
use klaub_core::semantic::{ChangeKind, MatchBand, MatchThresholds, SymbolChange};
use klaub_core::symbol::ParsedFile;
use klaub_match::{FilePair, match_symbols};
use klaub_parse::TreeSitterParser;

fn parse(source: &str) -> ParsedFile {
    TreeSitterParser::new()
        .parse(Language::Python, source)
        .unwrap()
}

fn path(p: &str) -> RepoPath {
    RepoPath::new(p).unwrap()
}

/// Matches one file whose content changed from `before` to `after`.
fn one_file(before: &str, after: &str) -> Vec<SymbolChange> {
    let (b, a, p) = (parse(before), parse(after), path("mod.py"));
    match_symbols(
        &[FilePair {
            path: &p,
            old_path: None,
            before: Some(&b),
            after: Some(&a),
        }],
        MatchThresholds::default(),
    )
}

fn kinds(changes: &[SymbolChange]) -> Vec<(String, ChangeKind)> {
    changes
        .iter()
        .map(|c| (c.display_name().to_owned(), c.kind))
        .collect()
}

const BASE: &str = r#"
def validate(token, strict=False):
    if not token:
        return False
    parts = token.split(".")
    for part in parts:
        if not part.isalnum():
            return False
    return len(parts) == 3 and not strict

def render(user):
    return "<b>" + user.name + "</b>"
"#;

#[test]
fn untouched_code_has_no_changes() {
    assert!(one_file(BASE, BASE).is_empty());
}

#[test]
fn a_body_edit_is_modified_and_the_neighbour_is_untouched() {
    let after = BASE.replace("len(parts) == 3", "len(parts) == 4");
    let changes = one_file(BASE, &after);
    assert_eq!(
        kinds(&changes),
        vec![("validate".to_owned(), ChangeKind::Modified)]
    );
    assert_eq!(changes[0].confidence.value(), 1.0);
}

#[test]
fn formatting_and_comments_are_format_only() {
    let after = BASE.replace(
        "parts = token.split(\".\")",
        "parts = token.split( \".\" )  # split it",
    );
    let changes = one_file(BASE, &after);
    assert_eq!(
        kinds(&changes),
        vec![("validate".to_owned(), ChangeKind::FormatOnly)]
    );
}

#[test]
fn blank_lines_between_definitions_are_not_a_unit() {
    let after = BASE.replace("\ndef render", "\n\n\ndef render");
    assert!(one_file(BASE, &after).is_empty());
}

#[test]
fn a_new_parameter_is_a_signature_change() {
    let after = BASE.replace(
        "def validate(token, strict=False):",
        "def validate(token, strict=False, leeway=0):",
    );
    let changes = one_file(BASE, &after);
    assert_eq!(
        kinds(&changes),
        vec![("validate".to_owned(), ChangeKind::SignatureChanged)]
    );
    assert!(
        changes[0]
            .evidence
            .iter()
            .any(|e| e.contains("declaration changed")),
        "{:?}",
        changes[0].evidence
    );
}

#[test]
fn a_visibility_change_is_a_signature_change() {
    let after = BASE.replace("validate", "_validate");
    let changes = one_file(BASE, &after);
    // The rename is found by similarity: same body, new (private) name.
    assert!(
        changes
            .iter()
            .any(|c| matches!(c.kind, ChangeKind::Renamed | ChangeKind::SignatureChanged)),
        "{changes:?}"
    );
}

#[test]
fn a_complete_rewrite_is_flagged_as_such() {
    let after = r#"
def validate(token, strict=False):
    import jwt
    try:
        claims = jwt.decode(token, KEY, algorithms=["HS256"])
    except jwt.InvalidTokenError:
        return None
    return claims

def render(user):
    return "<b>" + user.name + "</b>"
"#;
    let changes = one_file(BASE, after);
    assert_eq!(
        kinds(&changes),
        vec![("validate".to_owned(), ChangeKind::Rewritten)],
        "{:?}",
        changes[0].evidence
    );
}

#[test]
fn a_renamed_function_is_matched_to_its_old_self() {
    let after = BASE.replace("validate", "check_token");
    let changes = one_file(BASE, &after);
    assert_eq!(
        kinds(&changes),
        vec![("check_token".to_owned(), ChangeKind::Renamed)],
        "{changes:?}"
    );
    let change = &changes[0];
    assert_ne!(
        change.confidence.band(MatchThresholds::default()),
        MatchBand::Unresolved
    );
    assert_eq!(change.before.as_ref().unwrap().qualified_name, "validate");
    assert_eq!(change.after.as_ref().unwrap().qualified_name, "check_token");
}

#[test]
fn a_function_moved_into_a_class_is_a_move() {
    let after = r#"
class Validator:
    def validate(self, token, strict=False):
        if not token:
            return False
        parts = token.split(".")
        for part in parts:
            if not part.isalnum():
                return False
        return len(parts) == 3 and not strict

def render(user):
    return "<b>" + user.name + "</b>"
"#;
    let changes = one_file(BASE, after);
    let moved = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Moved)
        .unwrap_or_else(|| panic!("{changes:?}"));
    assert_eq!(moved.before.as_ref().unwrap().qualified_name, "validate");
    assert_eq!(
        moved.after.as_ref().unwrap().qualified_name,
        "Validator.validate"
    );
    assert!(
        changes
            .iter()
            .any(|c| c.kind == ChangeKind::Added && c.display_name() == "Validator"),
        "the new class itself is added"
    );
    assert!(
        !changes
            .iter()
            .any(|c| c.display_name() == "validate" && c.kind == ChangeKind::Removed)
    );
}

#[test]
fn added_and_removed_functions() {
    let after = format!(
        "{BASE}\ndef audit(event, actor, target, outcome):\n    record(event, actor, target, outcome)\n    return outcome\n"
    );
    let changes = one_file(BASE, &after);
    assert_eq!(
        kinds(&changes),
        vec![("audit".to_owned(), ChangeKind::Added)]
    );

    let without_render = BASE.split("def render").next().unwrap().to_owned();
    let changes = one_file(BASE, &without_render);
    assert_eq!(
        kinds(&changes),
        vec![("render".to_owned(), ChangeKind::Removed)]
    );
}

#[test]
fn a_near_miss_is_reported_but_not_claimed_as_a_move() {
    let before =
        "def alpha(a, b):\n    total = a + b\n    scale = total * 2\n    return scale - 1\n";
    // Shares part of the body with `alpha`, but is a different function.
    let after = "def beta(a, b, c):\n    total = a + b\n    scale = total * 3\n    check(c)\n    return log(scale)\n";
    let changes = one_file(before, after);
    let removed = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Removed)
        .unwrap();
    let added = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Added)
        .unwrap();
    assert_eq!(removed.display_name(), "alpha");
    assert_eq!(added.display_name(), "beta");
    let note = removed
        .evidence
        .iter()
        .chain(&added.evidence)
        .find(|e| e.contains("closest candidate"));
    assert!(
        note.is_some(),
        "a near miss should be explained: {:?} {:?}",
        removed.evidence,
        added.evidence
    );
}

#[test]
fn duplicate_names_are_paired_in_order() {
    let before = "def f():\n    return 1\n\ndef f():\n    return 2\n";
    let after = "def f():\n    return 1\n\ndef f():\n    return 3\n";
    let changes = one_file(before, after);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Modified);
    assert_eq!(
        changes[0].after.as_ref().unwrap().span.start_line,
        4,
        "the second definition is the one that changed"
    );
}

#[test]
fn moving_a_function_to_another_file() {
    let source = |name: &str| {
        format!(
            "def {name}(items, limit):\n    result = []\n    for item in items:\n        if item.ok and len(result) < limit:\n            result.append(item.value)\n    return result\n"
        )
    };
    let (old_a, new_a) = (
        parse(&format!(
            "{}\ndef keep():\n    return 1\n",
            source("collect")
        )),
        parse("def keep():\n    return 1\n"),
    );
    let (old_b, new_b) = (
        parse("def other():\n    return 2\n"),
        parse(&format!(
            "def other():\n    return 2\n\n{}",
            source("collect")
        )),
    );
    let (pa, pb) = (path("a.py"), path("b.py"));
    let changes = match_symbols(
        &[
            FilePair {
                path: &pa,
                old_path: None,
                before: Some(&old_a),
                after: Some(&new_a),
            },
            FilePair {
                path: &pb,
                old_path: None,
                before: Some(&old_b),
                after: Some(&new_b),
            },
        ],
        MatchThresholds::default(),
    );
    let moved = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Moved)
        .unwrap_or_else(|| panic!("{changes:?}"));
    assert_eq!(moved.before.as_ref().unwrap().file.as_str(), "a.py");
    assert_eq!(moved.after.as_ref().unwrap().file.as_str(), "b.py");
    assert!(
        moved
            .evidence
            .iter()
            .any(|e| e.contains("moved from a.py to b.py"))
    );
    assert_eq!(
        changes.len(),
        1,
        "no stray Added/Removed entries: {changes:?}"
    );
}

#[test]
fn a_renamed_file_keeps_its_symbols_matched() {
    let (before, after) = (parse(BASE), parse(&BASE.replace("== 3", "== 4")));
    let (old, new) = (path("old_name.py"), path("new_name.py"));
    let changes = match_symbols(
        &[FilePair {
            path: &new,
            old_path: Some(&old),
            before: Some(&before),
            after: Some(&after),
        }],
        MatchThresholds::default(),
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Modified);
    assert_eq!(
        changes[0].before.as_ref().unwrap().file.as_str(),
        "old_name.py"
    );
    assert_eq!(
        changes[0].after.as_ref().unwrap().file.as_str(),
        "new_name.py"
    );
}

#[test]
fn unique_names_found_elsewhere_are_moves_even_for_tiny_symbols() {
    let (old_a, new_a) = (parse("class Config:\n    pass\n"), parse(""));
    let (old_b, new_b) = (parse(""), parse("class Config:\n    pass\n"));
    let (pa, pb) = (path("a.py"), path("b.py"));
    let changes = match_symbols(
        &[
            FilePair {
                path: &pa,
                old_path: None,
                before: Some(&old_a),
                after: Some(&new_a),
            },
            FilePair {
                path: &pb,
                old_path: None,
                before: Some(&old_b),
                after: Some(&new_b),
            },
        ],
        MatchThresholds::default(),
    );
    assert_eq!(
        kinds(&changes),
        vec![("Config".to_owned(), ChangeKind::Moved)]
    );
    assert!(changes[0].confidence.value() >= 0.85);
}

#[test]
fn identical_ids_for_identical_inputs() {
    let after = BASE.replace("len(parts) == 3", "len(parts) == 4");
    let one = one_file(BASE, &after);
    let two = one_file(BASE, &after);
    assert_eq!(one, two);
}

#[test]
fn many_symbols_stay_fast() {
    let make = |tweak: bool| {
        (0..400)
            .map(|i| {
                let body = if tweak && i % 7 == 0 { format!("return {i} + 1") } else { format!("return {i}") };
                format!("def func_{i}(a, b):\n    value = a * {i} + b\n    if value > 10:\n        {body}\n    return value\n\n")
            })
            .collect::<String>()
    };
    let started = std::time::Instant::now();
    let changes = one_file(&make(false), &make(true));
    assert_eq!(changes.len(), 58);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}
