//! The flow documents under `fixtures/flows`, validated the way the reader validates them.
//!
//! The file name says what must happen: `current*` is accepted for this snapshot, `stale*` is
//! well formed but written for another, `symbol-missing*` is accepted with the step marked as not
//! found in the index, and `invalid-*` is refused with a reason.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use klaub_core::graph::SymbolKey;
use klaub_core::ids::{SnapshotId, WorktreeId};
use klaub_core::path::RepoPath;
use klaub_flow::{FlowContext, Validation, validate};

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/flows");
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&path).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

fn run(bytes: &[u8]) -> Validation {
    let snapshot = SnapshotId::parse("snap_abc123").unwrap();
    let worktree = WorktreeId::parse("wt_123").unwrap();
    let known = |p: &RepoPath| matches!(p.as_str(), "api/routes/auth.py" | "domain/auth.py");
    let symbols = |p: &RepoPath, s: &str| {
        matches!(
            (p.as_str(), s),
            ("api/routes/auth.py", "login") | ("domain/auth.py", "validate_credentials")
        )
    };
    let connected = |_: &SymbolKey, _: &SymbolKey| Some(true);
    validate(
        bytes,
        &FlowContext {
            snapshot_id: &snapshot,
            worktree_id: &worktree,
            base_oid: "oid_da_base",
            max_bytes: 1 << 20,
            knows_file: &known,
            symbol_exists: &symbols,
            connected: &connected,
        },
    )
}

#[test]
fn every_flow_fixture_is_judged_as_its_name_says() {
    let files = fixtures();
    assert!(files.len() >= 7, "the fixtures went missing: {files:?}");
    for (name, bytes) in &files {
        let verdict = run(bytes);
        let ok = match name.as_str() {
            n if n.starts_with("current") => {
                matches!(verdict, Validation::Current(ref f) if f.divergences.is_empty())
            }
            n if n.starts_with("stale") => matches!(verdict, Validation::Stale { .. }),
            n if n.starts_with("symbol-missing") => matches!(
                verdict,
                Validation::Current(ref f) if f.steps.iter().any(|s| s.symbol.is_some() && !s.symbol_found)
            ),
            n if n.starts_with("invalid-") => {
                matches!(verdict, Validation::Invalid(ref why) if !why.is_empty())
            }
            other => panic!(
                "{other}: a fixture name must start with current, stale, symbol-missing or invalid-"
            ),
        };
        assert!(ok, "{name} was judged {verdict:?}");
    }
}

#[test]
fn a_rejected_fixture_names_the_reason() {
    let reason = |name: &str| -> String {
        let (_, bytes) = fixtures().into_iter().find(|(n, _)| n == name).unwrap();
        match run(&bytes) {
            Validation::Invalid(why) => why.join(" | "),
            other => panic!("{name}: {other:?}"),
        }
    };
    assert!(reason("invalid-malformed.json").contains("not a valid flow document"));
    assert!(
        reason("invalid-path-traversal.json")
            .to_lowercase()
            .contains("path")
    );
    assert!(reason("invalid-schema-version.json").contains("schema"));
    assert!(!reason("invalid-terminal-text.json").is_empty());
}
