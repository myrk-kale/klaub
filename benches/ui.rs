//! Latency of the reader itself: one key press followed by the frame it causes.
//!
//! This is the other half of the latency targets: list and tree navigation under
//! 100 ms at p95 and the before/after toggle under 50 ms at p95. It drives the real `App` over a
//! generated repository and draws into a `TestBackend` of 120×40, so what it times is state update
//! plus layout plus widget rendering, not a terminal emulator. The `latency` bench covers
//! the analysis side. The numbers are for one machine and a synthetic corpus; see
//! `docs/benchmarks.md`.
//!
//! ```text
//! cargo bench --bench ui
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use klaub::adapters::git::GitSnapshotProvider;
use klaub::adapters::git::testing::{FixedClock, TempRepo};
use klaub::adapters::parse::TreeSitterParser;
use klaub::adapters::storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use klaub::app::{OpenOptions, Ports, ReviewSession};
use klaub::domain::config::Config;
use klaub::domain::ports::{Clipboard, IdGenerator};
use klaub::domain::snapshot::SnapshotRequest;
use klaub::tui::{App, draw, handle_key};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const FILES: usize = 200;
const FUNCTIONS: usize = 10;
const CHANGED: usize = 60;
const PRESSES: usize = 300;

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

fn source(file: usize, revision: u32) -> String {
    let mut text = String::new();
    let next = (file + 1) % FILES;
    writeln!(text, "from mod_{next} import fn_{next}_0\n").unwrap();
    for function in 0..FUNCTIONS {
        writeln!(text, "def fn_{file}_{function}(x):").unwrap();
        if revision > 0 {
            writeln!(text, "    y = x + {revision}").unwrap();
        }
        if function + 1 < FUNCTIONS {
            writeln!(text, "    return fn_{file}_{}(x)\n", function + 1).unwrap();
        } else {
            writeln!(text, "    return fn_{next}_0(x)\n").unwrap();
        }
    }
    text
}

fn app() -> (TempRepo, App) {
    let repo = TempRepo::new();
    for file in 0..FILES {
        repo.write(&format!("mod_{file}.py"), &source(file, 0));
    }
    repo.commit_all("initial");
    for file in 0..CHANGED {
        repo.write(&format!("mod_{file}.py"), &source(file, 1));
    }
    let config = Config::default();
    let clock = Arc::new(FixedClock::default());
    let ports = Ports {
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
    };
    let session = ReviewSession::open(
        ports,
        SnapshotRequest::worktree(repo.path()),
        &config,
        &OpenOptions::default(),
    )
    .unwrap();
    let app = App::new(
        session,
        Arc::new(TreeSitterParser::new()),
        None,
        None::<Arc<dyn Clipboard>>,
    );
    (repo, app)
}

fn press(app: &mut App, c: char) {
    handle_key(app, KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
}

/// One key press and the frame it produces.
fn step(app: &mut App, terminal: &mut Terminal<TestBackend>, code: KeyCode) -> Duration {
    let start = Instant::now();
    handle_key(app, KeyEvent::new(code, KeyModifiers::empty()));
    terminal.draw(|frame| draw(frame, app)).unwrap();
    start.elapsed()
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let last = sorted.len() - 1;
    sorted[(((last as f64) * p).ceil() as usize).min(last)]
}

fn row(what: &str, mut samples: Vec<Duration>, target_ms: u64) {
    samples.sort_unstable();
    let p95 = percentile(&samples, 0.95);
    let ms = |d: Duration| format!("{:.2}", d.as_secs_f64() * 1000.0);
    println!(
        "| {what} | {} | {} | {} | {} | target p95 {target_ms} ms: {} |",
        samples.len(),
        ms(percentile(&samples, 0.5)),
        ms(p95),
        ms(*samples.last().unwrap()),
        if p95 <= Duration::from_millis(target_ms) {
            "within"
        } else {
            "over"
        },
    );
}

fn main() {
    let (_repo, mut app) = app();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    println!(
        "Reader over {FILES} files, {CHANGED} changed, drawn at 120x40. Times are milliseconds.\n"
    );
    println!("| Operation | n | p50 | p95 | max | Target |");
    println!("| --- | --- | --- | --- | --- | --- |");

    // The queue: move down the list.
    let queue: Vec<Duration> = (0..PRESSES)
        .map(|_| step(&mut app, &mut terminal, KeyCode::Down))
        .collect();
    row("move in the queue (key + frame)", queue, 100);

    // The structure tree.
    press(&mut app, '2');
    let tree: Vec<Duration> = (0..PRESSES)
        .map(|_| step(&mut app, &mut terminal, KeyCode::Down))
        .collect();
    row("move in the structure tree (key + frame)", tree, 100);

    // The code view and the before/after toggle.
    press(&mut app, '1');
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
    );
    let toggle: Vec<Duration> = (0..PRESSES)
        .map(|_| step(&mut app, &mut terminal, KeyCode::Char('v')))
        .collect();
    row("toggle before/after (key + frame)", toggle, 50);
}
