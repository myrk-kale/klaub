//! The reader driven without a terminal: a real review session rendered into a `TestBackend`.
//!
//! Assertions look at the text the buffer holds, never at a full-screen snapshot, so a change in
//! spacing or column widths does not break them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use klaub_app::{OpenOptions, Ports, ReviewSession};
use klaub_core::config::Config;
use klaub_core::finding::Priority;
use klaub_core::ports::{AgentChannel, AgentTarget, Clipboard, IdGenerator, PrepareOutcome};
use klaub_core::review::Version;
use klaub_core::snapshot::SnapshotRequest;
use klaub_git::GitSnapshotProvider;
use klaub_git::testing::{FixedClock, TempRepo};
use klaub_parse::TreeSitterParser;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use klaub_tui::{App, Overlay, draw, handle_key};
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
        matches!(app.mode, klaub_tui::Mode::Code),
        "Enter should open the unit in Code"
    );

    press(&mut app, 'o');
    assert!(
        matches!(app.overlay, Some(Overlay::Contract(_))),
        "o in Code should open the contract overlay"
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
fn an_invalid_key_table_is_reported_before_the_reader_opens() {
    let mut keys = std::collections::BTreeMap::new();
    keys.insert("no_such_action".to_owned(), "e".to_owned());
    let error = klaub_tui::Keymap::from_overrides(&keys).unwrap_err();
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
    fn target(&self) -> klaub_core::Result<Option<AgentTarget>> {
        Ok(Some(AgentTarget {
            pane_id: "%7".to_owned(),
            command: "agent".to_owned(),
            description: "session klaub-1 · window agent · pane %7".to_owned(),
        }))
    }

    fn prepare_input(&self, text: &str) -> klaub_core::Result<PrepareOutcome> {
        self.pasted.lock().unwrap().push(text.to_owned());
        Ok(PrepareOutcome::Prepared)
    }
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
        Some(Overlay::Finish(klaub_tui::FinishStage::Deliver { selected, .. })) => {
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
