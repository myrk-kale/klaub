//! The reader driven without a terminal: a real review session rendered into a `TestBackend`.
//!
//! Assertions look at the text the buffer holds, never at a full-screen snapshot, so a change in
//! spacing or column widths does not break them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use klaub::adapters::git::GitSnapshotProvider;
use klaub::adapters::git::testing::{FixedClock, TempRepo};
use klaub::adapters::parse::TreeSitterParser;
use klaub::adapters::storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use klaub::app::{OpenOptions, Ports, ReviewSession};
use klaub::domain::config::Config;
use klaub::domain::finding::Priority;
use klaub::domain::ports::{AgentChannel, AgentTarget, Clipboard, IdGenerator, PrepareOutcome};
use klaub::domain::review::Version;
use klaub::domain::snapshot::SnapshotRequest;
use klaub::tui::{App, Mode, Overlay, draw, end_peek, handle_key, handle_mouse};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    let request = SnapshotRequest::worktree(repo.path());
    ReviewSession::open(
        ports(repo, &Config::default()),
        request,
        &Config::default(),
        &OpenOptions::default(),
    )
    .unwrap()
}

/// A function committed, then modified, so the queue has a before and an after.
fn changed_repo() -> TempRepo {
    let repo = TempRepo::new();
    repo.write("service.py", "def lookup(name):\n    return name\n");
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def lookup(name):\n    cleaned = name.strip()\n    return cleaned\n",
    );
    repo
}

fn reader(session: ReviewSession) -> App {
    App::new(
        session,
        Arc::new(TreeSitterParser::new()),
        None,
        None::<Arc<dyn Clipboard>>,
    )
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn press(app: &mut App, c: char) {
    handle_key(app, key(KeyCode::Char(c)));
}

fn type_word(app: &mut App, word: &str) {
    for c in word.chars() {
        press(app, c);
    }
}

/// Every symbol the buffer holds, in reading order.
fn screen(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn queue_is_ordered() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    let shown: Vec<(String, String)> = app
        .session
        .analysis()
        .queue
        .iter()
        .map(|item| (item.priority.label().to_owned(), item.unit.label.clone()))
        .collect();
    assert!(!shown.is_empty(), "the edit must produce a review queue");

    let rendered = screen(&mut app, 80, 24);
    let (first, label) = &shown[0];
    let at = rendered
        .find(first)
        .unwrap_or_else(|| panic!("{first} missing from:\n{rendered}"));
    assert!(
        rendered.contains(label),
        "the unit label is missing from:\n{rendered}"
    );
    if let Some((later, _)) = shown.iter().skip(1).find(|(priority, _)| priority != first) {
        let later_at = rendered
            .find(later)
            .unwrap_or_else(|| panic!("{later} missing from:\n{rendered}"));
        assert!(at < later_at, "{first} should be drawn before {later}");
    }
}

#[test]
fn toggle_before_after() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    let before = app.code.version;
    assert!(before.is_some(), "opening the unit should load a version");

    press(&mut app, 'v');
    assert_ne!(app.code.version, before, "v should flip the version");
    assert!(matches!(
        app.code.version,
        Some(Version::Before | Version::After)
    ));

    press(&mut app, 'v');
    assert_eq!(app.code.version, before, "a second v should flip it back");
}

#[test]
fn contract_overlay_shows_the_change() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        matches!(app.mode, klaub::tui::Mode::Code),
        "Enter should open the unit in Code"
    );

    press(&mut app, 'i');
    assert!(
        matches!(app.overlay, Some(Overlay::Contract(_))),
        "i in Code should open the contract overlay"
    );
    let shown = screen(&mut app, 100, 30);
    assert!(
        shown.contains("Contract"),
        "the panel should be titled {shown}"
    );
    assert!(
        shown.contains("lookup"),
        "the changed symbol's signature should be on screen: {shown}"
    );

    press(&mut app, 'x');
    assert!(app.overlay.is_none(), "any key should close the overlay");
}

/// Four functions in four files, a flow that branches at `start`, and the flow file on disk.
fn flow_app() -> (TempRepo, App) {
    let repo = TempRepo::new();
    for (file, name, body) in [
        ("a.py", "start", "mid()"),
        ("b.py", "mid", "finish()"),
        ("c.py", "finish", "1"),
        ("d.py", "side", "2"),
    ] {
        repo.write(file, &format!("def {name}():\n    return {body}\n"));
    }
    repo.commit_all("initial");
    for (file, name, body) in [
        ("a.py", "start", "mid() + 1"),
        ("b.py", "mid", "finish() + 1"),
        ("c.py", "finish", "2"),
        ("d.py", "side", "3"),
    ] {
        repo.write(file, &format!("def {name}():\n    return {body}\n"));
    }
    let mut session = open(&repo);
    let snapshot = session.analysis().snapshot.clone();
    let step = |id: &str, file: &str, symbol: &str| {
        serde_json::json!({"id": id, "file_path": file, "symbol": symbol, "note": "n",
            "changed": true, "confidence": "resolved"})
    };
    let edge = |from: &str, to: &str| serde_json::json!({"from": from, "to": to, "kind": "calls", "confidence": "resolved"});
    let document = serde_json::json!({
        "schema_version": 1, "snapshot_id": snapshot.id, "base_oid": snapshot.base_oid,
        "worktree_id": snapshot.worktree.id, "title": "Branching flow", "summary": "s",
        "steps": [step("start", "a.py", "start"), step("mid", "b.py", "mid"),
                  step("finish", "c.py", "finish"), step("side", "d.py", "side")],
        "edges": [edge("start", "mid"), edge("mid", "finish"), edge("start", "side")]
    });
    let path = repo.path().join(session.flow().expected_path.as_str());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, document.to_string()).unwrap();
    session.reload_flow().unwrap();
    (repo, reader(session))
}

#[test]
fn the_flow_is_an_indented_tree_with_a_breadcrumb_path() {
    let (_repo, mut app) = flow_app();
    press(&mut app, '0');
    let rows = app.flow_rows();
    assert_eq!(
        rows.iter()
            .map(|r| (r.row.step, r.row.depth))
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 1), (2, 2), (3, 1)],
        "depth-first, branches in document order"
    );

    press(&mut app, 'j');
    press(&mut app, 'j');
    let shown = screen(&mut app, 110, 30);
    assert!(
        shown.contains("path: start › mid › finish"),
        "the breadcrumb names every ancestor of the selected step:\n{shown}"
    );
}

#[test]
fn folding_hides_the_branches_and_keeps_the_step() {
    let (_repo, mut app) = flow_app();
    press(&mut app, '0');
    handle_key(&mut app, key(KeyCode::Left));
    assert_eq!(app.flow_rows().len(), 1, "start folded: only start remains");
    assert!(screen(&mut app, 110, 30).contains('▸'));

    handle_key(&mut app, key(KeyCode::Right));
    assert_eq!(app.flow_rows().len(), 4);
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(app.flow_rows().len(), 1, "space toggles");
}

#[test]
fn enter_opens_the_step_that_is_selected_even_after_folding() {
    let (_repo, mut app) = flow_app();
    press(&mut app, '0');
    press(&mut app, 'j');
    handle_key(&mut app, key(KeyCode::Left)); // fold `mid`, hiding `finish`
    assert_eq!(app.flow_rows().len(), 3);
    press(&mut app, 'j'); // now on `side`
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(app.mode, klaub::tui::Mode::Code));
    let unit = app.unit.clone().unwrap();
    let label = &app.session.analysis().item(&unit).unwrap().unit.label;
    assert!(label.contains("side"), "{label}");
}

/// Two changed functions in two files, so the queue has two units to walk between.
fn two_units() -> (
    TempRepo,
    App,
    klaub::domain::ids::UnitId,
    klaub::domain::ids::UnitId,
) {
    let repo = TempRepo::new();
    repo.write("a.py", "def first():\n    return 1\n");
    repo.write("b.py", "def second():\n    return 1\n");
    repo.commit_all("initial");
    repo.write("a.py", "def first():\n    return 2\n");
    repo.write("b.py", "def second():\n    return 2\n");
    let app = reader(open(&repo));
    let ids: Vec<_> = app
        .session
        .analysis()
        .queue
        .iter()
        .map(|item| item.unit.id.clone())
        .collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    let (one, two) = (ids[0].clone(), ids[1].clone());
    (repo, app, one, two)
}

#[test]
fn back_and_forward_walk_the_units_visited() {
    let (_repo, mut app, one, two) = two_units();
    app.open_unit(&one, None);
    app.open_unit(&two, None);
    assert_eq!(app.unit.as_ref(), Some(&two));

    press(&mut app, '<');
    assert_eq!(
        app.unit.as_ref(),
        Some(&one),
        "back returns to the unit before"
    );
    press(&mut app, '>');
    assert_eq!(
        app.unit.as_ref(),
        Some(&two),
        "forward returns to where back left"
    );

    press(&mut app, '<');
    press(&mut app, '<');
    assert_eq!(
        app.unit.as_ref(),
        Some(&one),
        "with nothing earlier, back stays where it is"
    );
    handle_key(&mut app, key(KeyCode::Esc)); // a stray key does not disturb the history
    app.open_unit(&two, None);
    press(&mut app, '>');
    assert_eq!(
        app.unit.as_ref(),
        Some(&two),
        "a new move forgets what was ahead"
    );
}

#[test]
fn o_reads_the_unit_inside_its_whole_file_and_stays_there() {
    let (_repo, mut app, one, _two) = two_units();
    app.open_unit(&one, None);
    let symbol_lines = app.code.view.as_ref().unwrap().lines.len();

    press(&mut app, 'o');
    assert_eq!(app.code.scope, Some(klaub::app::CodeScope::File));
    press(&mut app, 'o');
    assert_eq!(
        app.code.scope,
        Some(klaub::app::CodeScope::File),
        "o opens the file; unlike S it does not toggle back"
    );
    assert!(app.code.view.as_ref().unwrap().lines.len() >= symbol_lines);

    press(&mut app, 'S');
    assert_eq!(app.code.scope, Some(klaub::app::CodeScope::Symbol));
}

#[test]
fn a_finding_can_be_silenced_from_the_explanation_after_confirming() {
    let repo = TempRepo::new();
    repo.write("service.py", "def handle(value):\n    return int(value)\n");
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def handle(value):\n    try:\n        return int(value)\n    except Exception:\n        pass\n",
    );
    let mut app = reader(open(&repo));

    press(&mut app, 'w');
    let shown = screen(&mut app, 110, 40);
    assert!(shown.contains("risk.swallowed_error"), "{shown}");
    assert!(shown.contains("silence its rule"), "{shown}");

    press(&mut app, '1');
    assert!(matches!(app.overlay, Some(Overlay::Confirm(_))));
    assert!(
        app.session.muted_rules().is_empty(),
        "nothing is silenced before the answer"
    );
    let shown = screen(&mut app, 110, 40);
    assert!(
        shown.contains("Silence this rule for this review?"),
        "{shown}"
    );

    press(&mut app, 'y');
    assert_eq!(app.session.muted_rules(), ["risk.swallowed_error"]);

    press(&mut app, 'w');
    let shown = screen(&mut app, 110, 40);
    assert!(!shown.contains("(risk.swallowed_error)"), "{shown}");
    assert!(
        shown.contains("Silenced here: risk.swallowed_error"),
        "{shown}"
    );
}

#[test]
fn contract_overlay_lists_parameters_raises_and_io_for_a_symbol() {
    let repo = TempRepo::new();
    repo.write(
        "service.py",
        "import requests\n\ndef lookup(name):\n    return name\n",
    );
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "import requests\n\ndef lookup(name, strict: bool = False) -> str:\n    if strict and not name:\n        raise ValueError(\"empty\")\n    return requests.get(name)\n",
    );
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    press(&mut app, 'i');
    let shown = screen(&mut app, 110, 50);
    for expected in [
        "parameters",
        "+ strict: bool = False",
        "returns",
        "raises",
        "+ ValueError",
        "calls requests.get (looks like I/O)",
    ] {
        assert!(
            shown.contains(expected),
            "missing {expected:?} in:\n{shown}"
        );
    }
}

#[test]
fn contract_overlay_reads_a_dependency_manifest() {
    let repo = TempRepo::new();
    repo.write("requirements.txt", "requests==2.0\n");
    repo.commit_all("initial");
    repo.write("requirements.txt", "requests==2.31\nrich==13\n");
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    press(&mut app, 'i');
    assert!(matches!(app.overlay, Some(Overlay::Contract(_))));
    let shown = screen(&mut app, 110, 40);
    for expected in [
        "dependency manifest",
        "+ rich [requirements] ==13",
        "~ requests [requirements]: ==2.0 -> ==2.31",
    ] {
        assert!(
            shown.contains(expected),
            "missing {expected:?} in:\n{shown}"
        );
    }
}

#[test]
fn comment_empty_is_rejected() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    press(&mut app, 'c');
    assert!(
        matches!(app.overlay, Some(Overlay::Editor(_))),
        "c should open the comment editor"
    );

    handle_key(&mut app, key(KeyCode::Enter));
    match &app.overlay {
        Some(Overlay::Editor(editor)) => {
            assert!(editor.error.is_some(), "an empty comment must be rejected")
        }
        other => panic!("the editor should stay open, got {other:?}"),
    }

    type_word(&mut app, "parameterise");
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        app.overlay.is_none(),
        "a non-empty comment should be saved and close the editor"
    );
    assert!(
        !app.session.review().comments.is_empty(),
        "the saved comment should be on the review"
    );
}

#[test]
fn mark_reviewed() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    let unit = app
        .unit
        .clone()
        .expect("the reader selects the first queue item");
    assert!(!app.session.review().is_reviewed(&unit));

    press(&mut app, 'r');
    assert!(app.session.review().is_reviewed(&unit));
}

/// A reader whose configuration moves `toggle_reviewed` to `e`.
fn reader_with_keys(repo: &TempRepo, keys: &[(&str, &str)]) -> App {
    let mut config = Config::default();
    for (action, key) in keys {
        config.keys.insert((*action).to_owned(), (*key).to_owned());
    }
    let session = ReviewSession::open(
        ports(repo, &config),
        SnapshotRequest::worktree(repo.path()),
        &config,
        &OpenOptions::default(),
    )
    .unwrap();
    reader(session)
}

#[test]
fn a_configured_key_replaces_the_default_one() {
    let repo = changed_repo();
    let mut app = reader_with_keys(&repo, &[("toggle_reviewed", "e")]);
    let unit = app.unit.clone().unwrap();

    press(&mut app, 'r');
    assert!(
        !app.session.review().is_reviewed(&unit),
        "the old key no longer marks the unit"
    );
    press(&mut app, 'e');
    assert!(app.session.review().is_reviewed(&unit));
}

#[test]
fn the_help_lists_the_configured_key() {
    let repo = changed_repo();
    let mut app = reader_with_keys(&repo, &[("toggle_reviewed", "e")]);
    press(&mut app, '?');
    let help = screen(&mut app, 100, 60);
    let row = help
        .lines()
        .find(|line| line.contains("Mark reviewed"))
        .expect("the help lists the reviewed action")
        .replace('│', " ");
    assert!(
        row.trim_start().starts_with("e "),
        "the help row for reviewed units shows the new key: {row:?}"
    );
}

#[test]
fn a_moved_movement_key_moves_the_cursor_and_releases_the_old_one() {
    let repo = changed_repo();
    let mut app = reader_with_keys(&repo, &[("down", "z"), ("up", "Z")]);
    press(&mut app, '3');
    assert_eq!(app.mode, Mode::Code);
    let start = app.code.cursor;

    press(&mut app, 'j');
    assert_eq!(app.code.cursor, start, "the old key no longer moves");
    press(&mut app, 'z');
    assert_eq!(app.code.cursor, start + 1);
    press(&mut app, 'Z');
    assert_eq!(app.code.cursor, start);
}

#[test]
fn the_footer_names_the_moved_movement_key() {
    let repo = changed_repo();
    let mut app = reader_with_keys(&repo, &[("down", "z"), ("comment", "e")]);
    let text = screen(&mut app, 140, 30);
    assert!(text.contains("z/k move"), "{text}");
    assert!(text.contains("e comment"), "{text}");
    assert!(!text.contains("j/k"), "{text}");
}

#[test]
fn a_moved_follow_up_decision_key_does_not_leave_the_old_one_acting() {
    let repo = changed_repo();
    let mut app = reader_with_keys(&repo, &[("decide_resolved", "z")]);
    // The decision keys only act on a comment of the previous round; without one they do nothing,
    // so the check here is that the old key does not fall through to another action.
    app.go(Mode::Followup);
    press(&mut app, 'y');
    assert!(app.overlay.is_none());
    press(&mut app, 'z');
    assert!(app.overlay.is_none(), "there is no comment to decide on");
}

fn released(c: char) -> KeyEvent {
    KeyEvent::new_with_kind(
        KeyCode::Char(c),
        KeyModifiers::empty(),
        ratatui::crossterm::event::KeyEventKind::Release,
    )
}

#[test]
fn holding_the_peek_key_shows_the_previous_version_until_it_is_released() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    app.hold_supported = true;
    handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(app.mode, Mode::Code);
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::After)
    );

    press(&mut app, 'B');
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::Before)
    );
    // A held key repeats; that must not toggle it back.
    press(&mut app, 'B');
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::Before)
    );

    handle_key(&mut app, released('B'));
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::After)
    );
    assert!(app.peek.is_none());
}

#[test]
fn a_lost_focus_ends_a_peek_whose_release_will_never_arrive() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    app.hold_supported = true;
    handle_key(&mut app, key(KeyCode::Enter));
    press(&mut app, 'B');
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::Before)
    );
    end_peek(&mut app);
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::After)
    );
}

#[test]
fn without_key_releases_the_peek_key_explains_and_leaves_the_code_alone() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    press(&mut app, 'B');
    assert_eq!(
        app.code.version,
        Some(klaub::domain::review::Version::After)
    );
    let status = app.status.clone().expect("it says why").0;
    assert!(status.contains("does not report key releases"), "{status}");
    assert!(status.contains("v toggles"), "{status}");
}

fn click(app: &mut App, column: u16, row: u16) {
    handle_mouse(
        app,
        ratatui::crossterm::event::MouseEvent {
            kind: ratatui::crossterm::event::MouseEventKind::Down(
                ratatui::crossterm::event::MouseButton::Left,
            ),
            column,
            row,
            modifiers: KeyModifiers::empty(),
        },
    );
}

fn wheel(app: &mut App, down: bool) {
    let kind = if down {
        ratatui::crossterm::event::MouseEventKind::ScrollDown
    } else {
        ratatui::crossterm::event::MouseEventKind::ScrollUp
    };
    handle_mouse(
        app,
        ratatui::crossterm::event::MouseEvent {
            kind,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::empty(),
        },
    );
}

/// Three functions changed, so the queue has three rows to click.
fn three_changes() -> TempRepo {
    let repo = TempRepo::new();
    repo.write(
        "service.py",
        "def one(a):\n    return a\n\n\ndef two(a):\n    return a\n\n\ndef three(a):\n    return a\n",
    );
    repo.commit_all("initial");
    repo.write(
        "service.py",
        "def one(a):\n    return a + 1\n\n\ndef two(a):\n    return a + 2\n\n\ndef three(a):\n    return a + 3\n",
    );
    repo
}

#[test]
fn clicking_a_tab_switches_to_that_mode() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    let _ = screen(&mut app, 140, 30);
    let (area, mode) = *app
        .tab_hits
        .iter()
        .find(|(_, mode)| *mode == Mode::Coverage)
        .expect("the tab row is wide enough to show every tab");
    click(&mut app, area.x + 1, area.y);
    assert_eq!(app.mode, mode);
}

#[test]
fn clicking_a_row_selects_it_and_clicking_the_selected_row_opens_it() {
    let repo = three_changes();
    let mut app = reader(open(&repo));
    let _ = screen(&mut app, 140, 30);
    assert!(app.queue_view.len >= 3);
    let first = app.queue_selection();

    let area = app.queue_view.area;
    click(&mut app, area.x + 4, area.y + 2);
    assert_eq!(app.queue_view.selected, 2);
    assert_ne!(
        app.queue_selection(),
        first,
        "the unit under the cursor follows the click"
    );
    assert_eq!(app.mode, Mode::Queue, "one click only selects");

    let _ = screen(&mut app, 140, 30);
    click(&mut app, area.x + 4, area.y + 2);
    assert_eq!(
        app.mode,
        Mode::Code,
        "a second click on the same row opens it"
    );
}

#[test]
fn a_click_below_the_last_row_does_nothing() {
    let repo = three_changes();
    let mut app = reader(open(&repo));
    let _ = screen(&mut app, 140, 30);
    let area = app.queue_view.area;
    click(&mut app, area.x + 4, area.y + 20);
    assert_eq!(app.queue_view.selected, 0);
    assert_eq!(app.mode, Mode::Queue);
}

#[test]
fn the_wheel_moves_like_the_arrow_keys() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    let _ = screen(&mut app, 140, 30);
    let start = app.code.cursor;
    // The function is short, so the wheel's three lines stop at the last one.
    let last = app.code.view.as_ref().unwrap().lines.len() - 1;
    wheel(&mut app, true);
    assert_eq!(app.code.cursor, (start + 3).min(last));
    assert!(app.code.cursor > start);
    wheel(&mut app, false);
    assert_eq!(app.code.cursor, (start + 3).min(last).saturating_sub(3));
}

#[test]
fn clicking_a_code_line_puts_the_cursor_there() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    handle_key(&mut app, key(KeyCode::Enter));
    let _ = screen(&mut app, 140, 30);
    let area = app.code.area;
    click(&mut app, area.x + 10, area.y + 1);
    assert_eq!(app.code.cursor, 1);
}

#[test]
fn the_mouse_is_ignored_while_a_text_field_is_open() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    press(&mut app, '/');
    let before = app.queue_view;
    wheel(&mut app, true);
    click(&mut app, 5, 5);
    assert!(matches!(app.overlay, Some(Overlay::Search { .. })));
    assert_eq!(app.queue_view, before);
}

#[test]
fn an_invalid_key_table_is_reported_before_the_reader_opens() {
    let mut keys = std::collections::BTreeMap::new();
    keys.insert("no_such_action".to_owned(), "e".to_owned());
    let error = klaub::tui::Keymap::from_overrides(&keys).unwrap_err();
    assert!(error.to_string().contains("no_such_action"));
}

#[test]
fn the_storage_notice_explains_klaub_dir_and_closes_on_any_key() {
    let repo = changed_repo();
    let mut app = reader(open(&repo)).with_storage_notice();
    let shown = screen(&mut app, 100, 30);
    assert!(shown.contains("Where Klaub keeps its files"), "{shown}");
    assert!(shown.contains(".gitignore"), "{shown}");

    press(&mut app, 'z');
    assert!(app.overlay.is_none(), "any key closes the notice");
    let shown = screen(&mut app, 100, 30);
    assert!(!shown.contains("Where Klaub keeps its files"));
}

/// A comment carrying the sequences a terminal must never act on.
fn hostile_comment() -> String {
    let mut text = String::from("looks fine ");
    text.push('\u{1b}');
    text.push_str("[31m");
    text.push('\u{1b}');
    text.push_str("]0;pwned\u{7}");
    text.push('\u{202E}');
    text.push('\u{202A}');
    text.push_str(" end");
    text
}

#[test]
fn hostile_text_never_reaches_the_buffer() {
    let repo = changed_repo();
    let mut session = open(&repo);
    let unit = session.analysis().queue[0].unit.id.clone();
    session
        .add_comment(
            &unit,
            Version::After,
            None,
            &hostile_comment(),
            Some(Priority::P1),
        )
        .unwrap();

    let mut app = reader(session);
    press(&mut app, '5');
    let rendered = screen(&mut app, 120, 30);

    assert!(
        !rendered.contains('\u{1b}'),
        "an escape byte reached the buffer:\n{rendered}"
    );
    assert!(
        !rendered.contains('\u{202E}'),
        "a bidi override reached the buffer:\n{rendered}"
    );
    assert!(
        !rendered.contains('\u{202A}'),
        "a bidi embedding reached the buffer:\n{rendered}"
    );
    assert!(
        !rendered.contains("]0;"),
        "the OSC introducer reached the buffer:\n{rendered}"
    );
    assert!(
        rendered.contains("looks fine") && rendered.contains("end"),
        "the readable part of the comment should survive:\n{rendered}"
    );
}

#[test]
fn tiny_terminal() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    let rendered = screen(&mut app, 40, 12);
    assert!(
        rendered.contains("Terminal too small") || rendered.contains("too small"),
        "a terminal below the minimum should say so:\n{rendered}"
    );
}

#[derive(Default)]
struct FakeChannel {
    pasted: std::sync::Mutex<Vec<String>>,
}

impl AgentChannel for FakeChannel {
    fn target(&self) -> klaub::domain::Result<Option<AgentTarget>> {
        Ok(Some(AgentTarget {
            pane_id: "%7".to_owned(),
            command: "agent".to_owned(),
            description: "session klaub-1 · window agent · pane %7".to_owned(),
        }))
    }

    fn prepare_input(&self, text: &str) -> klaub::domain::Result<PrepareOutcome> {
        self.pasted.lock().unwrap().push(text.to_owned());
        Ok(PrepareOutcome::Prepared)
    }
}

/// A channel whose pane shows what the test says.
struct ScreenChannel {
    command: String,
    screen: String,
}

impl AgentChannel for ScreenChannel {
    fn target(&self) -> klaub::domain::Result<Option<AgentTarget>> {
        Ok(Some(AgentTarget {
            pane_id: "%7".to_owned(),
            command: self.command.clone(),
            description: "session klaub-1 · window agent · pane %7".to_owned(),
        }))
    }

    fn prepare_input(&self, _text: &str) -> klaub::domain::Result<PrepareOutcome> {
        Ok(PrepareOutcome::Prepared)
    }

    fn screen_tail(&self, _lines: usize) -> klaub::domain::Result<Option<String>> {
        Ok(Some(self.screen.clone()))
    }
}

struct BusyAware;

impl klaub::domain::harness::HarnessAdapter for BusyAware {
    fn id(&self) -> &'static str {
        "busy-aware"
    }
    fn name(&self) -> &'static str {
        "Busy-aware test harness"
    }
    fn recognizes(&self, command: &str) -> bool {
        command == "busy-aware"
    }
    fn readiness(&self, screen_tail: &str) -> klaub::domain::harness::Readiness {
        if screen_tail.contains("working") {
            klaub::domain::harness::Readiness::Busy("it says it is still working".to_owned())
        } else {
            klaub::domain::harness::Readiness::Ready
        }
    }
}

/// The text of the confirmation that `Prepare in agent` opens.
fn prepare_confirmation(command: &str, screen: &str, with_adapter: bool) -> String {
    let repo = changed_repo();
    let mut session = open(&repo);
    let unit = session.analysis().queue[0].unit.id.clone();
    session
        .add_comment(&unit, Version::After, None, "use a parameter", None)
        .unwrap();
    let channel = Arc::new(ScreenChannel {
        command: command.to_owned(),
        screen: screen.to_owned(),
    });
    let mut app = App::new(
        session,
        Arc::new(TreeSitterParser::new()),
        Some(channel),
        None::<Arc<dyn Clipboard>>,
    );
    if with_adapter {
        app = app.with_harnesses(
            klaub::domain::harness::HarnessRegistry::default().with(Arc::new(BusyAware)),
        );
    }
    press(&mut app, 'F');
    press(&mut app, 'y');
    // The delivery list: "Prepare in the agent" is the first option.
    handle_key(&mut app, key(KeyCode::Enter));
    screen_text(&mut app)
}

fn screen_text(app: &mut App) -> String {
    screen(app, 140, 40)
}

#[test]
fn the_confirmation_says_the_generic_adapter_knows_nothing_about_the_program() {
    let text = prepare_confirmation("codex", "working...", false);
    assert!(text.contains("harness: generic"), "{text}");
    assert!(!text.contains("still working"), "{text}");
}

#[test]
fn a_known_harness_adapter_can_warn_that_the_program_looks_busy() {
    let text = prepare_confirmation("busy-aware", "working on it", true);
    assert!(text.contains("harness: Busy-aware test harness"), "{text}");
    assert!(text.contains("it says it is still working"), "{text}");

    let idle = prepare_confirmation("busy-aware", "> ", true);
    assert!(idle.contains("harness: Busy-aware test harness"), "{idle}");
    assert!(!idle.contains("still working"), "{idle}");
}

#[test]
fn finish_never_sends_enter() {
    let repo = changed_repo();
    let mut session = open(&repo);
    let unit = session.analysis().queue[0].unit.id.clone();
    session
        .add_comment(&unit, Version::After, None, "use a parameter", None)
        .unwrap();

    let channel = Arc::new(FakeChannel::default());
    let mut app = App::new(
        session,
        Arc::new(TreeSitterParser::new()),
        Some(channel.clone()),
        None::<Arc<dyn Clipboard>>,
    );

    press(&mut app, 'F');
    press(&mut app, 'y');
    match &app.overlay {
        Some(Overlay::Finish(klaub::tui::FinishStage::Deliver { selected, .. })) => {
            assert_eq!(
                *selected, 0,
                "the first delivery option is Prepare in agent"
            );
        }
        other => panic!("finishing should offer delivery, got {other:?}"),
    }

    handle_key(&mut app, key(KeyCode::Enter));
    let Overlay::Confirm(confirm) = app.overlay.as_ref().unwrap() else {
        panic!("choosing Prepare in agent should ask for confirmation");
    };
    let shown = confirm.lines.join("\n");
    assert!(
        shown.contains("session klaub-1 · window agent · pane %7"),
        "the confirmation should name the destination: {shown}"
    );
    assert!(
        shown.contains("Klaub never presses Enter"),
        "the confirmation should state that nothing is sent: {shown}"
    );

    let rendered = screen(&mut app, 100, 30);
    assert!(
        rendered.contains("session klaub-1") && rendered.contains("never presses Enter"),
        "the confirmation should be on screen:\n{rendered}"
    );

    press(&mut app, 'y');
    let pasted = channel.pasted.lock().unwrap().clone();
    assert_eq!(
        pasted.len(),
        1,
        "prepare_input should be called exactly once"
    );
    assert!(
        !pasted[0].contains('\n') && !pasted[0].contains('\r'),
        "the prepared text must be a single line: {:?}",
        pasted[0]
    );
}

#[test]
fn drift_banner() {
    let repo = changed_repo();
    let mut app = reader(open(&repo));
    repo.write("service.py", "def lookup(name):\n    return name.upper()\n");
    app.session.check_drift().unwrap();

    let rendered = screen(&mut app, 100, 30);
    assert!(
        rendered.contains("worktree changed"),
        "a changed worktree should raise the banner:\n{rendered}"
    );
}
