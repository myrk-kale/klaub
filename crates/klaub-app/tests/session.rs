//! The application layer end to end: a real Git repository, the real parser and real storage.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_raw_string_hashes
)]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use klaub_app::{CodeScope, OpenOptions, Ports, ReviewSession};
use klaub_core::config::Config;
use klaub_core::finding::{Priority, QueueItem, QueueTag, Temporal};
use klaub_core::flow::FlowState;
use klaub_core::ids::UnitId;
use klaub_core::ports::{
    AgentChannel, AgentTarget, Clipboard, IdGenerator, PrepareOutcome, SnapshotProvider,
};
use klaub_core::review::{CommentStatus, Delivery, FollowupStatus, ReviewStatus, Version};
use klaub_core::semantic::LineMark;
use klaub_core::snapshot::{Drift, SnapshotRequest};
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

fn open(repo: &TempRepo, config: &Config, options: &OpenOptions) -> ReviewSession {
    let request = SnapshotRequest::worktree(repo.path());
    ReviewSession::open(ports(repo, config), request, config, options).unwrap()
}

const SERVICE: &str = "from db import find\n\ndef lookup(name):\n    return find(name)\n\ndef unrelated():\n    return 1\n";
const DB: &str = "def find(name):\n    return name\n";
const ROUTES: &str = "from fastapi import FastAPI\nfrom service import lookup\n\napp = FastAPI()\n\n@app.get(\"/users\")\ndef users(name):\n    return lookup(name)\n";
const TESTS: &str =
    "from service import lookup\n\ndef test_lookup():\n    assert lookup('a') == 'a'\n";

fn project() -> TempRepo {
    let repo = TempRepo::new();
    repo.write("service.py", SERVICE);
    repo.write("db.py", DB);
    repo.write("api.py", ROUTES);
    repo.write("tests/test_service.py", TESTS);
    repo.write("README.md", "# demo\n");
    repo.commit_all("initial");
    repo
}

fn item<'a>(session: &'a ReviewSession, label: &str) -> &'a QueueItem {
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
}

fn unit(session: &ReviewSession, label: &str) -> UnitId {
    item(session, label).unit.id.clone()
}

/// An edit that makes `find` unsafe and `lookup` richer.
fn edit(repo: &TempRepo) {
    repo.write(
        "db.py",
        "def find(name):\n    return db.execute(f\"SELECT * FROM t WHERE n = '{name}'\")\n",
    );
    repo.write(
        "service.py",
        &SERVICE.replace(
            "return find(name)",
            "cleaned = name.strip()\n    return find(cleaned)",
        ),
    );
}

#[test]
fn the_queue_puts_the_dangerous_change_first_and_explains_it() {
    let repo = project();
    edit(&repo);
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let queue = &session.analysis().queue;
    assert_eq!(
        queue[0].unit.label,
        "find",
        "{:?}",
        queue
            .iter()
            .map(|i| (&i.unit.label, i.priority))
            .collect::<Vec<_>>()
    );
    assert!(queue[0].priority <= Priority::P1);
    assert!(
        queue[0].findings.iter().any(
            |f| f.rule_id == "risk.sql_from_dynamic_text" && f.temporal == Temporal::Introduced
        )
    );
    assert!(queue[0].entrypoints >= 1, "reachable from the /users route");
    assert!(
        queue[0].score.parts.iter().all(|p| !p.reason.is_empty()),
        "every point has a reason"
    );

    assert!(
        queue.iter().all(|i| i.unit.label != "unrelated"),
        "untouched code is not in the queue"
    );
    assert!(queue.iter().all(|i| !i.unit.file.is_internal()));
    assert!(
        session.analysis().notes.is_empty(),
        "{:?}",
        session.analysis().notes
    );
}

#[test]
fn opening_twice_resumes_the_same_review_and_keeps_comments() {
    let repo = project();
    edit(&repo);
    let (review_id, comment_id) = {
        let mut session = open(&repo, &Config::default(), &OpenOptions::default());
        let find = unit(&session, "find");
        let id = session
            .add_comment(
                &find,
                Version::After,
                Some((2, 2)),
                "this builds SQL from user input\x1b[31m!",
                Some(Priority::P0),
            )
            .unwrap();
        assert!(session.toggle_reviewed(&unit(&session, "lookup")).unwrap());
        (session.review().review_id.clone(), id)
    };
    let again = open(&repo, &Config::default(), &OpenOptions::default());
    assert_eq!(
        again.review().review_id,
        review_id,
        "same snapshot, same review"
    );
    let comment = again.review().comment(&comment_id).unwrap();
    assert_eq!(
        comment.text, "this builds SQL from user input!",
        "sanitised on the way in"
    );
    assert!(again.review().is_reviewed(&unit(&again, "lookup")));
    assert_eq!(again.comments_on(&unit(&again, "find")).len(), 1);

    let file = fs::read_to_string(repo.path().join(again.review_file())).unwrap();
    assert!(
        file.contains("\"schema_version\": 1") && file.contains("this builds SQL"),
        "the comment is on disk"
    );
}

#[derive(Default)]
struct FakeChannel {
    refuse: bool,
    pasted: std::sync::Mutex<Vec<String>>,
}

impl AgentChannel for FakeChannel {
    fn target(&self) -> klaub_core::Result<Option<AgentTarget>> {
        Ok(None)
    }

    fn prepare_input(&self, text: &str) -> klaub_core::Result<PrepareOutcome> {
        if self.refuse {
            return Ok(PrepareOutcome::Unavailable(
                "the process in the agent pane was replaced".into(),
            ));
        }
        self.pasted.lock().unwrap().push(text.to_owned());
        Ok(PrepareOutcome::Prepared)
    }
}

#[derive(Default)]
struct FakeClipboard(std::sync::Mutex<Vec<String>>);

impl Clipboard for FakeClipboard {
    fn copy(&self, text: &str) -> klaub_core::Result<()> {
        self.0.lock().unwrap().push(text.to_owned());
        Ok(())
    }
}

#[test]
fn finishing_and_handing_the_feedback_to_the_agent_never_submits_it() {
    let repo = project();
    edit(&repo);
    let mut session = open(&repo, &Config::default(), &OpenOptions::default());
    assert!(session.finish().is_err(), "nothing to hand over yet");
    let find = unit(&session, "find");
    let id = session
        .add_comment(
            &find,
            Version::After,
            None,
            "use a parameterised query",
            None,
        )
        .unwrap();

    let refusing = FakeChannel {
        refuse: true,
        ..FakeChannel::default()
    };
    let outcome = session.prepare_in_agent(&refusing).unwrap();
    assert!(
        matches!(outcome, PrepareOutcome::Unavailable(ref why) if why.contains("replaced")),
        "{outcome:?}"
    );
    assert_eq!(
        session.review().comment(&id).unwrap().status,
        CommentStatus::Draft,
        "a refused paste changes nothing"
    );
    assert_eq!(
        session.review().status,
        ReviewStatus::Finished,
        "but the review is finished and saved"
    );

    let channel = FakeChannel::default();
    assert_eq!(
        session.prepare_in_agent(&channel).unwrap(),
        PrepareOutcome::Prepared
    );
    let pasted = channel.pasted.lock().unwrap().clone();
    assert_eq!(pasted.len(), 1);
    assert!(pasted[0].contains(&session.review_file()) && !pasted[0].contains('\n'));
    assert_eq!(
        session.review().comment(&id).unwrap().status,
        CommentStatus::Prepared
    );
    assert_eq!(
        session.review().feedback.as_ref().unwrap().delivery,
        Delivery::PreparedInAgent
    );

    session.confirm_submitted().unwrap();
    assert_eq!(
        session.review().comment(&id).unwrap().status,
        CommentStatus::Submitted
    );

    let clipboard = FakeClipboard::default();
    session.copy_feedback(&clipboard).unwrap();
    assert_eq!(clipboard.0.lock().unwrap()[0], pasted[0]);
}

#[test]
fn the_next_round_carries_comments_over_and_asks_for_a_human_decision() {
    let repo = project();
    edit(&repo);
    let comment_id = {
        let mut session = open(&repo, &Config::default(), &OpenOptions::default());
        let find = unit(&session, "find");
        let id = session
            .add_comment(
                &find,
                Version::After,
                Some((2, 2)),
                "use a parameterised query",
                None,
            )
            .unwrap();
        session.finish().unwrap();
        id
    };
    // The agent "fixes" the code (and moves it two lines down).
    repo.write("db.py", "# fixed\n\ndef find(name):\n    return db.execute(\"SELECT * FROM t WHERE n = %s\", (name,))\n");

    let mut second = open(&repo, &Config::default(), &OpenOptions::default());
    assert!(second.review().previous_review_id.is_some(), "a new round");
    let comment = second.review().comment(&comment_id).unwrap();
    assert_eq!(comment.followup_status, FollowupStatus::Pending);
    assert_eq!(comment.anchor.file_path.as_str(), "db.py");
    assert!(
        comment.anchor_confidence.value() >= 0.65,
        "the symbol is still there"
    );
    assert_ne!(
        comment.status,
        CommentStatus::Addressed,
        "nothing resolves a comment but the human"
    );

    second
        .decide_followup(
            &comment_id,
            FollowupStatus::Resolved,
            Some("parameters now".into()),
        )
        .unwrap();
    let decided = second.review().comment(&comment_id).unwrap();
    assert_eq!(decided.status, CommentStatus::Addressed);
    assert_eq!(decided.followup_decided_by.as_deref(), Some("human"));
}

#[test]
fn removing_the_commented_symbol_makes_the_anchor_stale_not_misplaced() {
    let repo = project();
    edit(&repo);
    let id = {
        let mut session = open(&repo, &Config::default(), &OpenOptions::default());
        let id = session
            .add_comment(
                &unit(&session, "find"),
                Version::After,
                None,
                "revisit",
                None,
            )
            .unwrap();
        session.finish().unwrap();
        id
    };
    repo.write(
        "db.py",
        "def something_else(a, b, c):\n    return a + b + c\n",
    );
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let comment = session.review().comment(&id).unwrap();
    assert_eq!(comment.status, CommentStatus::Stale);
    assert_eq!(comment.followup_status, FollowupStatus::Stale);
    assert!(comment.anchor_confidence.value() < 0.65);
}

#[test]
fn drift_is_noticed_and_refresh_relocates_without_silent_updates() {
    let repo = project();
    edit(&repo);
    let mut session = open(&repo, &Config::default(), &OpenOptions::default());
    let find = unit(&session, "find");
    let lookup = unit(&session, "lookup");
    session.toggle_reviewed(&find).unwrap();
    session.toggle_reviewed(&lookup).unwrap();
    let id = session
        .add_comment(
            &lookup,
            Version::After,
            None,
            "name the variable better",
            None,
        )
        .unwrap();
    let first_snapshot = session.analysis().snapshot.id.clone();

    assert_eq!(session.check_drift().unwrap(), Drift::Unchanged);
    std::thread::sleep(std::time::Duration::from_millis(20));
    repo.write(
        "db.py",
        "def find(name):\n    return db.execute(f\"SELECT * FROM t WHERE n = '{name}' LIMIT 1\")\n",
    );
    assert_eq!(session.check_drift().unwrap(), Drift::Changed);
    assert_eq!(
        session.analysis().snapshot.id,
        first_snapshot,
        "the snapshot does not change by itself"
    );

    session.refresh().unwrap();
    assert_ne!(session.analysis().snapshot.id, first_snapshot);
    assert_eq!(session.drift(), Drift::Unchanged);
    assert!(
        !session.review().is_reviewed(&find),
        "find changed since it was marked"
    );
    assert!(
        session.review().is_reviewed(&lookup),
        "lookup did not change"
    );
    assert!(
        session
            .review()
            .comment(&id)
            .unwrap()
            .anchor_confidence
            .value()
            >= 0.9
    );
    assert_eq!(session.review().snapshot_id, session.analysis().snapshot.id);
}

#[test]
fn the_reader_shows_both_versions_new_files_and_change_regions() {
    let repo = project();
    edit(&repo);
    repo.write("brand_new.py", "def fresh():\n    return 'new'\n");
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let parser = TreeSitterParser::new();
    let analysis = session.analysis();

    let find = unit(&session, "find");
    let after = analysis
        .code_view(&parser, &find, Version::After, CodeScope::Symbol)
        .unwrap();
    assert_eq!(
        after
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "def find(name):",
            "    return db.execute(f\"SELECT * FROM t WHERE n = '{name}'\")"
        ]
    );
    assert_eq!(after.lines[1].mark, LineMark::Changed);
    assert_eq!(after.lines[0].mark, LineMark::Same);
    assert!(
        after.lines[0]
            .spans
            .iter()
            .any(|s| s.class == klaub_core::symbol::HighlightClass::Keyword),
        "`def` is coloured"
    );
    assert_eq!(after.changes, vec![2]);
    assert!(after.other_version_exists);

    let before = analysis
        .code_view(&parser, &find, Version::Before, CodeScope::Symbol)
        .unwrap();
    assert_eq!(before.lines[1].text, "    return name");
    assert_eq!(before.symbol_lines, Some((1, 2)));

    let whole = analysis
        .code_view(&parser, &find, Version::After, CodeScope::File)
        .unwrap();
    assert_eq!(whole.lines.len(), 2);

    let fresh = unit(&session, "fresh");
    let no_before = analysis
        .code_view(&parser, &fresh, Version::Before, CodeScope::Symbol)
        .unwrap();
    assert!(no_before.lines.is_empty());
    assert!(
        no_before.notice.as_deref().unwrap().starts_with("NEW"),
        "{:?}",
        no_before.notice
    );
    assert!(!no_before.other_version_exists || no_before.notice.is_some());
    assert!(item(&session, "fresh").tags.contains(&QueueTag::NewFile));
}

#[test]
fn test_evidence_never_confuses_no_association_with_no_tests() {
    let repo = project();
    edit(&repo);
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let lookup = session.test_panel(&unit(&session, "lookup"));
    assert_eq!(
        lookup.links.len(),
        1,
        "test_lookup calls lookup and imports its module"
    );
    assert_eq!(lookup.status, EvidenceStatus::NotRun);
    assert!(!item(&session, "lookup").tags.contains(&QueueTag::NoTests));

    let find = session.test_panel(&unit(&session, "find"));
    assert_eq!(find.status, EvidenceStatus::NoAssociationFound);
    assert!(item(&session, "find").tags.contains(&QueueTag::NoTests));
}

#[test]
fn imported_reports_turn_not_run_into_passed_or_stale() {
    let repo = project();
    edit(&repo);
    fs::write(
        repo.path().join("junit.xml"),
        "<testsuites><testsuite><testcase classname=\"tests.test_service\" name=\"test_lookup\" time=\"0.01\"/></testsuite></testsuites>",
    )
    .unwrap();
    // Make the report newer than the edits.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(
        repo.path().join("junit.xml"),
        "<testsuites><testsuite><testcase classname=\"tests.test_service\" name=\"test_lookup\" time=\"0.01\"/></testsuite></testsuites>",
    )
    .unwrap();
    let mut config = Config::default();
    config.tests.report_paths = vec!["junit.xml".into()];
    let session = open(&repo, &config, &OpenOptions::default());
    let panel = session.test_panel(&unit(&session, "lookup"));
    assert_eq!(
        panel.status,
        EvidenceStatus::Passed,
        "{:?}",
        session.reports()
    );
    assert_eq!(panel.results.len(), 1);
    assert!(!session.reports().stale);

    // An edit after the report makes it stale.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    repo.write(
        "service.py",
        &SERVICE.replace(
            "return find(name)",
            "cleaned = name.strip().lower()\n    return find(cleaned)",
        ),
    );
    let later = open(&repo, &config, &OpenOptions::default());
    assert_eq!(
        later.test_panel(&unit(&later, "lookup")).status,
        EvidenceStatus::Stale
    );
}

#[test]
fn the_flow_diagram_is_current_stale_invalid_or_missing_and_always_optional() {
    let repo = project();
    edit(&repo);
    let mut session = open(&repo, &Config::default(), &OpenOptions::default());
    assert_eq!(session.flow().state, FlowState::Missing);
    assert!(
        session.flow().request_prompt.contains("klaub-flow")
            && !session.flow().request_prompt.contains('\n')
    );
    assert_eq!(session.review().flow.as_ref().unwrap().status, "missing");

    let snapshot = session.analysis().snapshot.clone();
    let document = |snapshot_id: &str, file: &str| {
        serde_json::json!({
            "schema_version": 1, "snapshot_id": snapshot_id, "base_oid": snapshot.base_oid, "worktree_id": snapshot.worktree.id,
            "title": "How a lookup reaches the database", "summary": "route to query",
            "steps": [
                {"id": "route", "file_path": "api.py", "symbol": "users", "note": "receives", "changed": false, "confidence": "resolved"},
                {"id": "svc", "file_path": "service.py", "symbol": "lookup", "note": "cleans", "changed": true, "confidence": "resolved"},
                {"id": "db", "file_path": file, "symbol": "find", "note": "queries", "changed": true, "confidence": "heuristic"}
            ],
            "edges": [
                {"from": "route", "to": "svc", "kind": "calls", "confidence": "resolved"},
                {"from": "svc", "to": "db", "kind": "calls", "confidence": "resolved"}
            ]
        })
    };
    let path = repo.path().join(session.flow().expected_path.as_str());
    fs::create_dir_all(path.parent().unwrap()).unwrap();

    fs::write(&path, document(snapshot.id.as_str(), "db.py").to_string()).unwrap();
    session.reload_flow().unwrap();
    assert_eq!(
        session.flow().state,
        FlowState::Current,
        "{:?}",
        session.flow().issues
    );
    assert!(
        session.flow().flow.as_ref().unwrap().divergences.is_empty(),
        "{:?}",
        session.flow().flow.as_ref().unwrap().divergences
    );

    fs::write(&path, document("snap_oldoldold", "db.py").to_string()).unwrap();
    session.reload_flow().unwrap();
    assert_eq!(session.flow().state.label(), "stale");
    assert!(
        session.flow().flow.is_some(),
        "a stale flow can still be consulted"
    );

    fs::write(
        &path,
        document(snapshot.id.as_str(), "../../etc/passwd").to_string(),
    )
    .unwrap();
    session.reload_flow().unwrap();
    assert_eq!(session.flow().state.label(), "invalid");
    assert!(
        session
            .flow()
            .issues
            .iter()
            .any(|i| i.contains("not a relative path")),
        "{:?}",
        session.flow().issues
    );
    assert!(session.flow().flow.is_none());
}

#[test]
fn coverage_counts_marks_and_warns_without_blocking() {
    let repo = project();
    edit(&repo);
    let mut session = open(&repo, &Config::default(), &OpenOptions::default());
    let before = session.coverage();
    assert_eq!(before.reviewed, 0);
    assert!(before.total >= 2);
    let warnings = session.warnings_before_finish();
    assert!(warnings.iter().any(|w| w.contains("P0/P1")), "{warnings:?}");

    for item in session.analysis().queue.clone() {
        session.toggle_reviewed(&item.unit.id).unwrap();
    }
    let after = session.coverage();
    assert_eq!(after.reviewed, after.total);
    assert!(
        session
            .warnings_before_finish()
            .iter()
            .all(|w| !w.contains("P0/P1"))
    );
    // Finishing is never blocked by warnings.
    session
        .add_comment(&unit(&session, "find"), Version::After, None, "x", None)
        .unwrap();
    assert!(session.finish().is_ok());
}

#[test]
fn a_brief_file_is_imported_and_edited_by_the_human() {
    let repo = project();
    edit(&repo);
    fs::write(repo.path().join("brief.md"), "# Harden lookups\n\nKeep the HTTP contract.\n\n- [ ] No new dependencies\n- [x] Queries are parameterised\n").unwrap();
    let mut session = open(
        &repo,
        &Config::default(),
        &OpenOptions {
            brief_file: Some("brief.md".into()),
            ..OpenOptions::default()
        },
    );
    assert_eq!(
        session.brief().intent_summary.as_deref(),
        Some("Keep the HTTP contract.")
    );
    assert_eq!(session.brief().acceptance_criteria.len(), 2);
    assert!(
        session
            .brief()
            .acceptance_criteria
            .iter()
            .all(|c| c.status == klaub_core::review::CriterionStatus::NotChecked)
    );
    let first = session.brief().acceptance_criteria[0].id.clone();
    session
        .set_criterion(
            &first,
            klaub_core::review::CriterionStatus::Verified,
            Some("checked requirements"),
        )
        .unwrap();
    let reopened = open(&repo, &Config::default(), &OpenOptions::default());
    assert_eq!(
        reopened.brief().acceptance_criteria[0].status,
        klaub_core::review::CriterionStatus::Verified
    );

    let outside = OpenOptions {
        brief_file: Some("../../etc/passwd".into()),
        ..OpenOptions::default()
    };
    assert!(
        ReviewSession::open(
            ports(&repo, &Config::default()),
            SnapshotRequest::worktree(repo.path()),
            &Config::default(),
            &outside
        )
        .is_err()
    );
}

#[test]
fn the_cache_is_used_on_the_second_analysis() {
    let repo = project();
    edit(&repo);
    let config = Config::default();
    let shared = ports(&repo, &config);
    let request = SnapshotRequest::worktree(repo.path());
    let first = klaub_app::analyze(&shared, &request, &config).unwrap();
    let second = klaub_app::analyze(&shared, &request, &config).unwrap();
    assert!(first.cache_misses > 0);
    assert_eq!(second.cache_misses, 0, "everything was cached");
    assert!(second.cache_hits >= first.cache_misses);
    assert_eq!(
        first.queue.iter().map(|i| &i.unit.id).collect::<Vec<_>>(),
        second.queue.iter().map(|i| &i.unit.id).collect::<Vec<_>>()
    );
}

#[test]
fn exports_follow_the_documented_schema() {
    let repo = project();
    edit(&repo);
    let mut session = open(&repo, &Config::default(), &OpenOptions::default());
    session
        .add_comment(
            &unit(&session, "find"),
            Version::After,
            Some((2, 2)),
            "parameterise this",
            Some(Priority::P0),
        )
        .unwrap();
    let value = session.export_json();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["comment_count"], 1);
    assert_eq!(value["comments_file"], session.review_file());
    assert!(value["finding_delta"]["introduced"].as_u64().unwrap() >= 1);
    assert_eq!(value["flow"]["status"], "missing");
    assert!(session.export_markdown().contains("parameterise this"));
}

#[test]
fn non_code_changes_are_reviewed_as_files_and_nothing_is_dropped() {
    let repo = project();
    repo.write("README.md", "# demo\n\nmore\n");
    repo.write(
        "web/package.json",
        "{\"dependencies\": {\"left-pad\": \"1.0.0\"}}\n",
    );
    repo.write_bytes("logo.png", &[0x89, b'P', 0, 1]);
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let labels: Vec<&str> = session
        .analysis()
        .queue
        .iter()
        .map(|i| i.unit.label.as_str())
        .collect();
    for expected in ["README.md", "web/package.json", "logo.png"] {
        assert!(
            labels.contains(&expected),
            "{expected} missing from {labels:?}"
        );
    }
    let manifest = item(&session, "web/package.json");
    assert!(
        manifest
            .findings
            .iter()
            .any(|f| f.rule_id == "risk.dependency_manifest")
    );
    assert!(item(&session, "logo.png").tags.contains(&QueueTag::Unknown));
    let parser = TreeSitterParser::new();
    let view = session
        .analysis()
        .code_view(
            &parser,
            &item(&session, "logo.png").unit.id,
            Version::After,
            CodeScope::File,
        )
        .unwrap();
    assert!(view.lines.is_empty() && view.notice.is_some());
}

#[test]
fn ids_and_paths_in_the_queue_are_stable_between_runs() {
    let repo = project();
    edit(&repo);
    let one = open(&repo, &Config::default(), &OpenOptions::default());
    let first: Vec<_> = one
        .analysis()
        .queue
        .iter()
        .map(|i| (i.unit.id.clone(), i.priority, i.score.total()))
        .collect();
    drop(one);
    let two = open(&repo, &Config::default(), &OpenOptions::default());
    let second: Vec<_> = two
        .analysis()
        .queue
        .iter()
        .map(|i| (i.unit.id.clone(), i.priority, i.score.total()))
        .collect();
    assert_eq!(first, second);
    let _ = Path::new(".");
    let provider = GitSnapshotProvider::new(Arc::new(FixedClock::default()));
    assert!(provider.discover(repo.path()).is_ok());
}

// ---- a symbol that moves to another file -------------------------------------------------------

/// `compute` leaves `old.py` and appears, unchanged, in `new.py`.
fn moved_between_files() -> TempRepo {
    let repo = TempRepo::new();
    let body = "def compute(a, b):\n    total = a + b\n    return total * 2\n";
    repo.write("old.py", &format!("{body}\n\ndef keep(x):\n    return x\n"));
    repo.write("new.py", "def other():\n    return 0\n");
    repo.commit_all("initial");
    repo.write("old.py", "def keep(x):\n    return x\n");
    repo.write("new.py", &format!("def other():\n    return 0\n\n\n{body}"));
    repo
}

fn moved_unit(session: &ReviewSession) -> UnitId {
    session
        .analysis()
        .queue
        .iter()
        .find(|item| {
            session
                .analysis()
                .change_of(&item.unit.id)
                .is_some_and(|c| c.kind == klaub_core::semantic::ChangeKind::Moved)
        })
        .map(|item| item.unit.id.clone())
        .expect("the move is a unit of the queue")
}

#[test]
fn a_symbol_moved_to_another_file_opens_without_reading_the_wrong_parse() {
    let repo = moved_between_files();
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let unit = moved_unit(&session);
    let change = session.analysis().change_of(&unit).unwrap();
    assert_eq!(change.before.as_ref().unwrap().file.as_str(), "old.py");
    assert_eq!(change.after.as_ref().unwrap().file.as_str(), "new.py");
}

#[test]
fn the_contract_of_a_moved_symbol_has_both_sides_from_their_own_files() {
    let repo = moved_between_files();
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let unit = moved_unit(&session);
    let contract = session.contract(&unit).expect("a contract");
    let before = contract.before_signature.as_deref().unwrap_or_default();
    let after = contract.after_signature.as_deref().unwrap_or_default();
    assert!(before.contains("def compute(a, b)"), "{before}");
    assert!(after.contains("def compute(a, b)"), "{after}");
}

#[test]
fn the_old_code_of_a_moved_symbol_is_read_from_the_file_it_left() {
    let repo = moved_between_files();
    let session = open(&repo, &Config::default(), &OpenOptions::default());
    let unit = moved_unit(&session);
    let parser = TreeSitterParser::new();

    let before = session
        .analysis()
        .code_view(&parser, &unit, Version::Before, CodeScope::Symbol)
        .unwrap();
    assert_eq!(before.path.as_str(), "old.py");
    let shown: Vec<_> = before.lines.iter().map(|l| l.text.as_str()).collect();
    assert!(
        shown.iter().any(|l| l.contains("total = a + b")),
        "the old body is shown: {shown:?}"
    );

    let after = session
        .analysis()
        .code_view(&parser, &unit, Version::After, CodeScope::Symbol)
        .unwrap();
    assert_eq!(after.path.as_str(), "new.py");
    assert!(after.lines.iter().any(|l| l.text.contains("total = a + b")));
}
