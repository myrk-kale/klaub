//! Latency of the review pipeline on a generated corpus.
//!
//! The project sets initial latency targets as hypotheses to be measured, not facts. This harness
//! measures them. It prints a table of p50, p95 and max per operation and sets the target beside
//! each one. It never fails: the numbers depend on the machine,
//! so a target that is missed is reported, not enforced.
//!
//! The default corpus is synthetic (Python files whose functions call the next one and a function
//! in the next file), so what is measured is the pipeline's shape on a repository of a given size,
//! not the speed of any real project. `docs/benchmarks.md` says what that does and does not tell
//! you. Point `KLAUB_BENCH_REPO` at a real repository to time the same operations on it.
//!
//! ```text
//! cargo bench -p klaub-app --bench latency
//! KLAUB_BENCH_SIZES=small,medium,large cargo bench -p klaub-app --bench latency
//! KLAUB_BENCH_REPO=~/src/flask,~/src/django KLAUB_BENCH_BASE=HEAD~40 \
//!     cargo bench -p klaub-app --bench latency
//! ```
//!
//! A real repository is cloned into a temporary directory first, so the review files Klaub writes
//! never land in it. The review is the branch scope: `KLAUB_BENCH_BASE` against `HEAD`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use klaub_app::{OpenOptions, Ports, ReviewSession};
use klaub_core::config::Config;
use klaub_core::ids::UnitId;
use klaub_core::ports::{AnalysisCache, IdGenerator};
use klaub_core::review::Version;
use klaub_core::snapshot::{Scope, SnapshotRequest};
use klaub_git::GitSnapshotProvider;
use klaub_git::testing::{FixedClock, TempRepo};
use klaub_parse::TreeSitterParser;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};

/// A corpus size: how many files exist, how many functions each has, how many files change.
struct Size {
    name: &'static str,
    files: usize,
    functions: usize,
    changed: usize,
}

const SIZES: &[Size] = &[
    Size {
        name: "small",
        files: 20,
        functions: 5,
        changed: 8,
    },
    Size {
        name: "medium",
        files: 200,
        functions: 10,
        changed: 60,
    },
    Size {
        name: "large",
        files: 1000,
        functions: 10,
        changed: 200,
    },
];

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

fn source(file: usize, size: &Size, revision: u32) -> String {
    let mut text = String::new();
    let next = (file + 1) % size.files;
    writeln!(text, "from mod_{next} import fn_{next}_0\n").unwrap();
    for function in 0..size.functions {
        writeln!(text, "def fn_{file}_{function}(x):").unwrap();
        if revision > 0 {
            writeln!(text, "    y = x + {revision}").unwrap();
        }
        if function + 1 < size.functions {
            writeln!(text, "    return fn_{file}_{}(x)\n", function + 1).unwrap();
        } else {
            writeln!(text, "    return fn_{next}_0(x)\n").unwrap();
        }
    }
    text
}

fn corpus(size: &Size) -> TempRepo {
    let repo = TempRepo::new();
    for file in 0..size.files {
        repo.write(&format!("mod_{file}.py"), &source(file, size, 0));
    }
    repo.commit_all("initial");
    edit(&repo, size, 1);
    repo
}

/// Rewrites the first `changed` files at `revision`.
fn edit(repo: &TempRepo, size: &Size, revision: u32) {
    for file in 0..size.changed {
        repo.write(&format!("mod_{file}.py"), &source(file, size, revision));
    }
}

fn ports(root: &Path, config: &Config, cache: Arc<dyn AnalysisCache>) -> Ports {
    let clock = Arc::new(FixedClock::default());
    Ports {
        snapshots: Arc::new(GitSnapshotProvider::new(clock.clone())),
        parser: Arc::new(TreeSitterParser::new()),
        reviews: Arc::new(JsonReviewRepository::new(root)),
        flows: Arc::new(FsFlowRepository::new(root, &config.flows.directory).unwrap()),
        cache,
        files: Arc::new(FsFileReader),
        config: Arc::new(FsConfigSource::new(None)),
        clock,
        ids: Arc::new(CountingIds::default()),
        runner: None,
    }
}

fn open(repo: &TempRepo, cache: Arc<dyn AnalysisCache>) -> ReviewSession {
    open_request(repo.path(), SnapshotRequest::worktree(repo.path()), cache)
}

fn open_request(
    root: &Path,
    request: SnapshotRequest,
    cache: Arc<dyn AnalysisCache>,
) -> ReviewSession {
    let config = Config::default();
    ReviewSession::open(
        ports(root, &config, cache),
        request,
        &config,
        &OpenOptions::default(),
    )
    .unwrap()
}

/// Sorted samples with the percentile lookups the table needs.
struct Samples(Vec<Duration>);

impl Samples {
    fn new(mut samples: Vec<Duration>) -> Self {
        samples.sort_unstable();
        Self(samples)
    }

    fn percentile(&self, p: f64) -> Duration {
        let last = self.0.len().saturating_sub(1);
        let index = ((last as f64) * p).ceil() as usize;
        self.0[index.min(last)]
    }
}

fn millis(duration: Duration) -> String {
    format!("{:.2}", duration.as_secs_f64() * 1000.0)
}

fn row(size: &str, what: &str, samples: &Samples, target: Option<(Duration, &str)>) {
    let verdict = match target {
        Some((limit, text)) => {
            let p95 = samples.percentile(0.95);
            format!("{text}: {}", if p95 <= limit { "within" } else { "over" })
        }
        None => "no target".to_owned(),
    };
    println!(
        "| {size} | {what} | {} | {} | {} | {} | {verdict} |",
        samples.0.len(),
        millis(samples.percentile(0.5)),
        millis(samples.percentile(0.95)),
        millis(*samples.0.last().unwrap()),
    );
}

fn time<T>(mut run: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    let value = run();
    let elapsed = start.elapsed();
    drop(value);
    elapsed
}

fn units(session: &ReviewSession) -> Vec<UnitId> {
    session
        .analysis()
        .queue
        .iter()
        .map(|item| item.unit.id.clone())
        .collect()
}

fn measure(size: &Size) {
    let repo = corpus(size);
    let label = format!("{} files, {} changed", size.files, size.changed);

    // Cold: every parse is computed, nothing is cached.
    let cold = Samples::new(
        (0..5)
            .map(|_| {
                let cache: Arc<dyn AnalysisCache> = Arc::new(SqliteCache::in_memory().unwrap());
                time(|| open(&repo, cache.clone()))
            })
            .collect(),
    );
    row(
        &label,
        "first queue, cold cache",
        &cold,
        Some((
            Duration::from_secs(2),
            "target 2 s for a small or medium review",
        )),
    );

    // Warm: the parses are in the cache, as on the second review of a worktree.
    let cache: Arc<dyn AnalysisCache> = Arc::new(SqliteCache::in_memory().unwrap());
    let mut session = open(&repo, cache.clone());
    let warm = Samples::new(
        (0..9)
            .map(|_| time(|| open(&repo, cache.clone())))
            .collect(),
    );
    row(&label, "first queue, warm cache", &warm, None);

    // Before/after: the work behind `v`, for every unit in both directions.
    let parser = TreeSitterParser::new();
    let toggle: Vec<Duration> = units(&session)
        .iter()
        .flat_map(|unit| [Version::Before, Version::After].map(|v| (unit.clone(), v)))
        .map(|(unit, version)| {
            time(|| {
                session
                    .analysis()
                    .code_view(&parser, &unit, version, klaub_app::CodeScope::Symbol)
            })
        })
        .collect();
    row(
        &label,
        "toggle before/after (view rebuild)",
        &Samples::new(toggle),
        Some((Duration::from_millis(50), "target p95 50 ms")),
    );

    // Incremental: one file edited, then a new snapshot (the cache holds the other parses).
    let refresh = Samples::new(
        (2..12)
            .map(|revision| {
                repo.write("mod_0.py", &source(0, size, revision));
                time(|| session.refresh().unwrap())
            })
            .collect(),
    );
    row(
        &label,
        "refresh after a one-file edit",
        &refresh,
        Some((Duration::from_secs(3), "target p95 3 s")),
    );
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The same operations on a real repository: `base` against `HEAD`, read-only (no refresh, since
/// that needs an edit). The repository is cloned first so nothing is written into it.
fn measure_repo(path: &Path, base: &str) {
    let scratch = tempfile::tempdir().unwrap();
    let clone = scratch.path().join("repo");
    git(
        scratch.path(),
        &["clone", "--quiet", &path.display().to_string(), "repo"],
    );
    let files = git(&clone, &["ls-files"]).lines().count();
    let request = SnapshotRequest {
        scope: Scope::Branch,
        base: base.to_owned(),
        ..SnapshotRequest::worktree(&clone)
    };
    let name = path.file_name().map_or_else(
        || "repository".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );

    let mut session = open_request(
        &clone,
        request.clone(),
        Arc::new(SqliteCache::in_memory().unwrap()),
    );
    let changed = session.analysis().snapshot.files.len();
    let queue = session.analysis().queue.len();
    let label = format!("{name}: {files} files, {changed} changed, {queue} units");

    let cold = Samples::new(
        (0..3)
            .map(|_| {
                let cache: Arc<dyn AnalysisCache> = Arc::new(SqliteCache::in_memory().unwrap());
                time(|| open_request(&clone, request.clone(), cache.clone()))
            })
            .collect(),
    );
    row(
        &label,
        "first queue, cold cache",
        &cold,
        Some((
            Duration::from_secs(2),
            "target 2 s for a small or medium review",
        )),
    );

    let cache: Arc<dyn AnalysisCache> = Arc::new(SqliteCache::in_memory().unwrap());
    session = open_request(&clone, request.clone(), cache.clone());
    let warm = Samples::new(
        (0..5)
            .map(|_| time(|| open_request(&clone, request.clone(), cache.clone())))
            .collect(),
    );
    row(&label, "first queue, warm cache", &warm, None);

    let parser = TreeSitterParser::new();
    let toggle: Vec<Duration> = units(&session)
        .iter()
        .take(500)
        .flat_map(|unit| [Version::Before, Version::After].map(|v| (unit.clone(), v)))
        .map(|(unit, version)| {
            time(|| {
                session
                    .analysis()
                    .code_view(&parser, &unit, version, klaub_app::CodeScope::Symbol)
            })
        })
        .collect();
    row(
        &label,
        "toggle before/after (view rebuild)",
        &Samples::new(toggle),
        Some((Duration::from_millis(50), "target p95 50 ms")),
    );
}

fn main() {
    let repos: Vec<String> = std::env::var("KLAUB_BENCH_REPO")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let sizes = std::env::var("KLAUB_BENCH_SIZES").ok();
    // A real repository replaces the synthetic corpus unless the sizes are asked for as well.
    let wanted: Vec<String> = match (&sizes, repos.is_empty()) {
        (Some(sizes), _) => sizes.split(',').map(|s| s.trim().to_owned()).collect(),
        (None, true) => vec!["small".to_owned(), "medium".to_owned()],
        (None, false) => Vec::new(),
    };
    if repos.is_empty() {
        println!(
            "Synthetic corpus, one machine, {} sample(s) per row at most. Times are milliseconds.\n",
            10
        );
    } else {
        println!("Real repositories, one machine. Times are milliseconds.\n");
    }
    println!("| Corpus | Operation | n | p50 | p95 | max | Target |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for size in SIZES.iter().filter(|s| wanted.iter().any(|w| w == s.name)) {
        measure(size);
    }
    let base = std::env::var("KLAUB_BENCH_BASE").unwrap_or_else(|_| "HEAD~30".to_owned());
    for repo in &repos {
        let path = std::fs::canonicalize(repo).expect("KLAUB_BENCH_REPO names an existing path");
        measure_repo(&path, &base);
    }
}
