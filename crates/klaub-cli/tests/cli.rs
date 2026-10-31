//! End-to-end tests of the `klaub` binary.
//!
//! Directories are redirected into a temporary folder through `KLAUB_*_DIR`, `TMUX` is
//! removed from the environment, and sessions use their own private tmux servers, which
//! each test stops before it ends.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use serde_json::Value;

fn find_tmux() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KLAUB_TMUX_PATH") {
        return Some(PathBuf::from(path));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("tmux"))
        .find(|path| path.is_file())
}

struct Sandbox {
    root: tempfile::TempDir,
    tmux: Option<PathBuf>,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            tmux: find_tmux(),
        }
    }

    fn klaub(&self) -> Command {
        let mut command = cargo_bin_cmd!("klaub");
        command
            .env("HOME", self.root.path())
            .env("KLAUB_CACHE_DIR", self.root.path().join("cache"))
            .env("KLAUB_STATE_DIR", self.root.path().join("state"))
            .env("KLAUB_CONFIG_DIR", self.root.path().join("config"))
            .env_remove("TMUX")
            .env_remove("KLAUB_SESSION")
            .env_remove("KLAUB_LOG")
            .current_dir(self.root.path());
        if let Some(tmux) = &self.tmux {
            command.env("KLAUB_TMUX_PATH", tmux);
        }
        command
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.klaub().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("stdout must be pure JSON")
    }

    fn worktree(&self) -> &Path {
        self.root.path()
    }
}

impl Drop for Sandbox {
    /// Stops any session a failing test left behind, so no private tmux server outlives the run.
    fn drop(&mut self) {
        let Ok(output) = self
            .klaub()
            .args(["session", "list", "--format", "json"])
            .output()
        else {
            return;
        };
        let Ok(Value::Array(rows)) = serde_json::from_slice::<Value>(&output.stdout) else {
            return;
        };
        for row in rows {
            if let Some(id) = row["id"].as_str() {
                let _ = self.klaub().args(["session", "kill", "--yes", id]).output();
            }
        }
    }
}

fn require_tmux(sandbox: &Sandbox) -> bool {
    if sandbox.tmux.is_none() {
        eprintln!("skipping: tmux is not installed");
    }
    sandbox.tmux.is_some()
}

#[test]
fn prints_its_version() {
    cargo_bin_cmd!("klaub")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("klaub "));
}

#[test]
fn version_json_is_pure_json() {
    let sandbox = Sandbox::new();
    let value = sandbox.json(&["version", "--format", "json"]);
    assert_eq!(value["name"], "klaub");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn help_lists_the_main_commands() {
    cargo_bin_cmd!("klaub")
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("wrap")
                .and(predicate::str::contains("review"))
                .and(predicate::str::contains("doctor")),
        );
}

#[test]
fn no_arguments_shows_usage_and_fails() {
    cargo_bin_cmd!("klaub")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn completions_are_generated() {
    for shell in ["bash", "zsh", "fish"] {
        cargo_bin_cmd!("klaub")
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("klaub"));
    }
}

#[test]
fn doctor_reports_each_check_as_json() {
    let sandbox = Sandbox::new();
    if !require_tmux(&sandbox) {
        return;
    }
    let value = sandbox.json(&["doctor", "--format", "json"]);
    assert_eq!(value["ok"], true);
    let names: Vec<&str> = value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for expected in ["klaub", "git", "tmux", "terminal", "state dir", "sessions"] {
        assert!(
            names.contains(&expected),
            "missing check {expected}: {names:?}"
        );
    }
}

#[test]
fn doctor_does_not_leak_the_home_directory() {
    let sandbox = Sandbox::new();
    let output = sandbox.klaub().arg("doctor").output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let home = sandbox.root.path().to_string_lossy().into_owned();
    // HOME is the sandbox, so every path below it must be abbreviated.
    assert!(!text.contains(&format!("{home}/")), "{text}");
}

#[test]
fn doctor_fails_for_an_unusable_explicit_tmux() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .klaub()
        .args([
            "--tmux-path",
            "/definitely/not/tmux",
            "doctor",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], false);
}

#[test]
fn wrap_refuses_to_nest_inside_tmux() {
    let sandbox = Sandbox::new();
    sandbox
        .klaub()
        .env("TMUX", "/tmp/tmux-1000/default,1234,0")
        .args(["wrap", "--detach", "sh"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "nested sessions are not supported",
        ));
}

#[test]
fn wrap_reports_a_missing_command_before_starting_anything() {
    let sandbox = Sandbox::new();
    if !require_tmux(&sandbox) {
        return;
    }
    sandbox
        .klaub()
        .args(["wrap", "--detach", "definitely-not-a-real-command-xyz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in PATH"));
    sandbox
        .klaub()
        .args(["session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no sessions"));
}

#[test]
fn a_session_can_be_started_listed_and_ended() {
    let sandbox = Sandbox::new();
    if !require_tmux(&sandbox) {
        return;
    }
    let output = sandbox
        .klaub()
        .args(["wrap", "--detach", "--", "sh", "-c", "sleep 120"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let id = stdout
        .split_whitespace()
        .find(|word| word.starts_with("ses_"))
        .unwrap_or_else(|| panic!("no session id in {stdout:?}"))
        .to_owned();

    let listed = sandbox.json(&["session", "list", "--format", "json"]);
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id.as_str())
        .expect("the session is listed");
    assert_eq!(row["state"], "running", "{row}");
    assert_eq!(row["command"], serde_json::json!(["sh", "-c", "sleep 120"]));
    let recorded = row["worktree"].as_str().unwrap();
    assert!(
        Path::new(recorded).ends_with(sandbox.worktree().file_name().unwrap()),
        "{recorded}"
    );

    sandbox
        .klaub()
        .args(["session", "kill", "--yes", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("ended"));
    sandbox
        .klaub()
        .args(["session", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no sessions"));
}

/// What the session's private tmux server says about its panes (for failure messages).
fn pane_report(tmux: &Path, socket: &str) -> String {
    let run = |args: &[&str]| {
        let output = std::process::Command::new(tmux)
            .args(["-L", socket])
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr)
    };
    let panes = run(&[
        "list-panes",
        "-a",
        "-F",
        "#{window_name} id=#{pane_id} dead=#{pane_dead} status=#{pane_dead_status} cmd=#{pane_current_command}",
    ]);
    let review = run(&["capture-pane", "-p", "-t", "review"]);
    format!("{panes}\n--- review pane ---\n{review}")
}

#[test]
fn the_review_window_stays_open() {
    let sandbox = Sandbox::new();
    if !require_tmux(&sandbox) {
        return;
    }
    let output = sandbox
        .klaub()
        .args(["wrap", "--detach", "--", "sh", "-c", "sleep 120"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let id = stdout
        .split_whitespace()
        .find(|w| w.starts_with("ses_"))
        .unwrap()
        .to_owned();
    let socket = format!("klaub-{}", id.trim_start_matches("ses_"));
    let tmux = sandbox.tmux.clone().unwrap();

    // The reviewer takes over the pane's terminal; give it time to fail if it is going to.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let report = pane_report(&tmux, &socket);
    assert!(
        report.contains("review id=") && !report.contains("review id=%1 dead=1"),
        "the review window died:\n{report}"
    );
    sandbox
        .klaub()
        .args(["session", "kill", "--yes", &id])
        .assert()
        .success();
}

#[test]
fn killing_requires_confirmation_without_a_terminal() {
    let sandbox = Sandbox::new();
    if !require_tmux(&sandbox) {
        return;
    }
    let output = sandbox
        .klaub()
        .args(["wrap", "--detach", "--", "sh", "-c", "sleep 120"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let id = stdout
        .split_whitespace()
        .find(|w| w.starts_with("ses_"))
        .unwrap()
        .to_owned();

    sandbox
        .klaub()
        .args(["session", "kill", &id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
    assert_eq!(
        sandbox
            .json(&["session", "list", "--format", "json"])
            .as_array()
            .unwrap()
            .len(),
        1,
        "nothing was ended"
    );

    sandbox
        .klaub()
        .args(["session", "kill", "--yes", &id])
        .assert()
        .success();
}

#[test]
fn attaching_without_sessions_explains_what_to_do() {
    let sandbox = Sandbox::new();
    sandbox
        .klaub()
        .args(["session", "attach"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("klaub wrap"));
}

#[test]
fn the_reviewer_explains_that_it_needs_a_terminal() {
    let sandbox = Sandbox::new();
    sandbox
        .klaub()
        .args(["review"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("needs a terminal"));
}

#[test]
fn the_reviewer_needs_a_known_session() {
    let sandbox = Sandbox::new();
    sandbox
        .klaub()
        .args(["review", "--session", "ses_doesnotexist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot load session"));
}
