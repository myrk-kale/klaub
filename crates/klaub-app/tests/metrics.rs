//! Measured evidence for the claims the specification asks to be documented: symbol matching,
//! temporal classification, test association and comment-anchor stability.
//!
//! Each number below is an assertion against a repository whose answer is known, so it is
//! reproducible rather than a claim. These are fixture measurements, not a corpus and not a
//! benchmark; see `docs/benchmarks.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_raw_string_hashes
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use klaub_app::{Analysis, OpenOptions, Ports, ReviewSession};
use klaub_core::config::Config;
use klaub_core::finding::Temporal;
use klaub_core::ids::UnitId;
use klaub_core::ports::IdGenerator;
use klaub_core::review::Version;
use klaub_core::semantic::ChangeKind;
use klaub_core::snapshot::SnapshotRequest;
use klaub_core::testing::EvidenceStatus;
use klaub_git::GitSnapshotProvider;
use klaub_git::testing::{FixedClock, TempRepo};
use klaub_parse::TreeSitterParser;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};

#[derive(Default)]
struct CountingIds(AtomicU64);

impl IdGenerator for CountingIds {
    fn random_hex(&self) -> String {
        format!(
            "{:012x}",
            self.0.fetch_add(1, Ordering::SeqCst) + 0xa0_0000_0000
        )
    }
}

fn ports(repo: &TempRepo, config: &Config) -> Ports {
    let clock = Arc::new(FixedClock::default());
    Ports {
        snapshots: Arc::new(GitSnapshotProvider::new(clock.clone())),
        parser: Arc::new(TreeSitterParser::new()),
        reviews: Arc::new(JsonReviewRepository::new(repo.path())),
        flows: Arc::new(FsFlowRepository::new(repo.path(), &config.flows.directory).unwrap()),
        cache: Arc::new(SqliteCache::in_memory().unwrap()),
        files: Arc::new(FsFileReader),
        config: Arc::new(FsConfigSource::new(None)),
        clock,
        ids: Arc::new(CountingIds::default()),
        runner: None,
    }
}

fn open(repo: &TempRepo) -> ReviewSession {
    let config = Config::default();
    let request = SnapshotRequest::worktree(repo.path());
    ReviewSession::open(
        ports(repo, &config),
        request,
        &config,
        &OpenOptions::default(),
    )
    .unwrap()
}

/// Change kinds of every symbol unit in the analysis, keyed by the name the symbol has after the
/// change (or before it, for a symbol that no longer exists).
fn kinds(analysis: &Analysis) -> Vec<(String, ChangeKind)> {
    let mut out: Vec<(String, ChangeKind)> = analysis
        .files
        .iter()
        .flat_map(|f| &f.analysis.changes)
        .map(|c| {
            let name = c
                .after
                .as_ref()
                .or(c.before.as_ref())
                .map(|r| r.qualified_name.clone())
                .unwrap_or_default();
            (name, c.kind)
        })
        .collect();
    out.sort();
    out
}

fn unit(session: &ReviewSession, label: &str) -> UnitId {
    session
        .analysis()
        .queue
        .iter()
        .find(|i| i.unit.label == label)
        .unwrap_or_else(|| {
            panic!(
                "{label} not in {:?}",
                session
                    .analysis()
                    .queue
                    .iter()
                    .map(|i| &i.unit.label)
                    .collect::<Vec<_>>()
            )
        })
        .unit
        .id
        .clone()
}

/// Precision 1.0 and recall 1.0 on a set of four: one rename, one body edit, one addition, one
/// untouched function.
///
/// Of the three functions that changed, all three are classified as the kind they actually are, and
/// the function whose text did not change produces no change entry at all.
#[test]
fn symbol_match_precision_one_of_one_recall_one_of_one_on_four_functions() {
    let repo = TempRepo::new();
    repo.write(
        "service.py",
        "def kept():\n    return 1\n\ndef renamed():\n    return 2\n\ndef edited():\n    return 3\n",
    );
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def kept():\n    return 1\n\ndef renamed_now():\n    return 2\n\ndef edited():\n    return 3 + 1\n\ndef added():\n    return 4\n",
    );

    let got = kinds(open(&repo).analysis());
    assert_eq!(
        got,
        vec![
            ("added".to_owned(), ChangeKind::Added),
            ("edited".to_owned(), ChangeKind::Modified),
            ("renamed_now".to_owned(), ChangeKind::Renamed),
        ],
        "expected exactly one added, one modified and one renamed; the untouched `kept` must be absent"
    );
}

/// Temporal classification on one unit: a finding present only after the edit is `introduced`, and
/// nothing on that unit is `preexisting`.
///
/// The pattern is the one `risk.swallowed_error` flags: an `except` block whose body only discards
/// the error (`pass`). It is added to a function that previously had none.
#[test]
fn one_swallowed_error_is_introduced_and_none_preexist() {
    let repo = TempRepo::new();
    repo.write("service.py", "def handle(value):\n    return int(value)\n");
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def handle(value):\n    try:\n        return int(value)\n    except Exception:\n        pass\n",
    );

    let session = open(&repo);
    let findings = &session
        .analysis()
        .item(&unit(&session, "handle"))
        .unwrap()
        .findings;
    let swallowed: Vec<_> = findings
        .iter()
        .filter(|f| f.rule_id == "risk.swallowed_error")
        .collect();
    assert_eq!(swallowed.len(), 1, "{findings:?}");
    assert_eq!(swallowed[0].temporal, Temporal::Introduced);
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.temporal == Temporal::Preexisting)
            .count(),
        0,
        "{findings:?}"
    );
}

/// Test association: a test that imports and calls a function is reported as associated, and a
/// function that nothing calls is reported as having no association.
#[test]
fn a_calling_test_associates_and_an_uncalled_function_does_not() {
    let repo = TempRepo::new();
    repo.write(
        "service.py",
        "def lookup(name):\n    return name\n\ndef unused():\n    return 1\n",
    );
    repo.write(
        "tests/test_service.py",
        "from service import lookup\n\ndef test_lookup():\n    assert lookup('a') == 'a'\n",
    );
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def lookup(name):\n    return name.strip()\n\ndef unused():\n    return 1 + 1\n",
    );

    let session = open(&repo);
    let lookup = session.test_panel(&unit(&session, "lookup"));
    assert_ne!(
        lookup.status,
        EvidenceStatus::NoAssociationFound,
        "a test that imports and calls `lookup` must associate: {lookup:?}"
    );
    assert!(
        !lookup.links.is_empty(),
        "the association must name the test: {lookup:?}"
    );

    let unused = session.test_panel(&unit(&session, "unused"));
    assert_eq!(unused.status, EvidenceStatus::NoAssociationFound);
    assert!(unused.links.is_empty(), "{unused:?}");
}

/// Comment-anchor stability: a comment on a function survives an edit that pushes the function
/// several lines down, and its anchor line moves with the function instead of going stale.
#[test]
fn a_comment_anchor_follows_its_function_when_lines_are_inserted_above_it() {
    let repo = TempRepo::new();
    repo.write("service.py", "def target():\n    return 1\n");
    repo.commit_all("initial");
    repo.write("service.py", "def target():\n    return 1 + 1\n");

    let (before, comment_id) = {
        let mut session = open(&repo);
        let target = unit(&session, "target");
        let start = session
            .analysis()
            .change_of(&target)
            .unwrap()
            .after
            .as_ref()
            .unwrap()
            .span
            .start_line;
        let id = session
            .add_comment(
                &target,
                Version::After,
                Some((start, start)),
                "check this",
                None,
            )
            .unwrap();
        (start, id)
    };

    repo.write(
        "service.py",
        "def padding():\n    return 0\n\n\ndef target():\n    return 1 + 1\n",
    );
    repo.commit_all("shift the function down");
    repo.write(
        "service.py",
        "def padding():\n    return 0\n\n\ndef target():\n    return 1 + 1 + 1\n",
    );

    let session = open(&repo);
    let comment = session.review().comment(&comment_id).unwrap();
    let after = comment.anchor.line_range.unwrap().0;
    assert_ne!(
        comment.status,
        klaub_core::review::CommentStatus::Stale,
        "the comment must still be attached to `target`"
    );
    assert!(
        after > before,
        "the anchor must move down with the function: was line {before}, now {after}"
    );
}
