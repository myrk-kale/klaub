//! Shared helpers for the tmux integration tests.
//!
//! Every test starts its own private tmux server on a unique socket and stops
//! only that server when it finishes, so tests never touch the developer's tmux.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use klaub::adapters::tmux::client::TmuxClient;
use klaub::adapters::tmux::registry::{SessionRecord, SessionRegistry};
use klaub::adapters::tmux::session::{LaunchSpec, client_for, launch};
use klaub::domain::ids::SessionId;
use klaub::domain::timestamp::Timestamp;

/// A stand-in for a coding agent's TUI. It switches the terminal to non-canonical mode
/// without echo or CR translation, enables bracketed paste (unless `NO_BRACKETED`
/// is set), announces itself and records every byte it receives in `$1`.
pub const HARNESS: &str = r#"
stty -icanon -echo -icrnl -ixon min 1 time 0
if [ -z "$NO_BRACKETED" ]; then printf '\033[?2004h'; fi
echo HARNESS-READY
dd bs=1 count=4000 of="$1" 2>/dev/null
"#;

/// Finds a tmux to test with: `KLAUB_TMUX_PATH`, else the first `tmux` on `PATH`.
pub fn find_tmux() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KLAUB_TMUX_PATH") {
        return Some(PathBuf::from(path));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("tmux"))
        .find(|path| path.is_file())
}

/// Returns the tmux to use or prints why the test is skipped.
pub fn tmux_or_skip() -> Option<PathBuf> {
    let tmux = find_tmux();
    if tmux.is_none() {
        eprintln!("skipping: no tmux found (set KLAUB_TMUX_PATH or install tmux)");
    }
    tmux
}

/// Polls `condition` until it holds or `timeout` elapses.
pub fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A running Klaub session in a temporary directory.
pub struct Fixture {
    pub temp: tempfile::TempDir,
    pub registry: SessionRegistry,
    pub record: SessionRecord,
    pub log: PathBuf,
    pub review_command: Vec<String>,
    pub tmux: PathBuf,
}

impl Fixture {
    /// Starts a session whose agent runs `script` with the log path as `$1`.
    pub fn start(tmux: &Path, script: &str, env_no_bracketed: bool) -> Self {
        Self::start_with_review(tmux, script, env_no_bracketed, |_, _| {
            ["sleep", "600"].map(str::to_owned).to_vec()
        })
    }

    /// Like [`Fixture::start`], with a custom review command built from the registry directory
    /// and the session id.
    pub fn start_with_review(
        tmux: &Path,
        script: &str,
        env_no_bracketed: bool,
        review: impl FnOnce(&Path, &SessionId) -> Vec<String>,
    ) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new(temp.path().join("sessions"));
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let id = SessionId::parse(format!("ses_t{}n{n}", std::process::id())).unwrap();
        let log = temp.path().join("agent.log");
        let worktree = temp.path().join("work");
        std::fs::create_dir_all(&worktree).unwrap();

        let body = if env_no_bracketed {
            format!("NO_BRACKETED=1; export NO_BRACKETED\n{script}")
        } else {
            script.to_owned()
        };
        let agent: Vec<String> = ["sh", "-c", &body, "harness", log.to_str().unwrap()]
            .map(str::to_owned)
            .to_vec();
        let review_command = review(&temp.path().join("sessions"), &id);
        let record = launch(
            &registry,
            &LaunchSpec {
                session_id: &id,
                worktree: &worktree,
                agent_command: &agent,
                review_command: &review_command,
                klaub_exe: Path::new("/usr/local/bin/klaub"),
                prefix: "C-b",
                tmux_binary: tmux,
                created_at: Timestamp::from_unix(1_700_000_000).unwrap(),
            },
        )
        .unwrap();
        Self {
            temp,
            registry,
            record,
            log,
            review_command,
            tmux: tmux.to_path_buf(),
        }
    }

    pub fn client(&self) -> TmuxClient {
        client_for(&self.record)
    }

    /// Waits until the fake harness has printed its ready marker.
    pub fn wait_ready(&self) {
        let client = self.client();
        let pane = self.record.agent.pane_id.clone();
        assert!(
            wait_until(Duration::from_secs(10), || client
                .capture_pane(&pane)
                .is_ok_and(|text| text.contains("HARNESS-READY"))),
            "the fake harness never became ready"
        );
    }

    /// The bytes the harness has received so far.
    pub fn received(&self) -> Vec<u8> {
        std::fs::read(&self.log).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Stops only this fixture's private server.
        let _ = self.client().kill_server();
    }
}

/// Finds `needle` in `haystack`.
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
