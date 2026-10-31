//! End-to-end tests of the `klaub` binary.
//!
//! Directories are redirected into a temporary folder through `KLAUB_*_DIR`, `TMUX` is
//! removed from the environment, and sessions use their own private tmux servers, which
//! each test stops before it ends.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command as Process;

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

/// Makes `dir` a Git repository with one commit, which is the least a review can open.
fn init_repo(dir: &Path) {
    let git = |args: &[&str]| {
        let status = Process::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.name", "Klaub Test"]);
    git(&["config", "user.email", "test@klaub.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("README.md"), "reviewed by the test\n").unwrap();
    git(&["add", "README.md"]);
    git(&["commit", "-q", "-m", "initial"]);
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
    // The reviewer analyses the worktree it starts in, so that directory has to be a repository.
    init_repo(sandbox.worktree());
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

// ---- diagnostics -----------------------------------------------------------------------------------

fn doctor_checks(sandbox: &Sandbox, configure: impl FnOnce(&mut Command)) -> Vec<Value> {
    let mut command = sandbox.klaub();
    command.args(["doctor", "--format", "json"]);
    configure(&mut command);
    let output = command.output().unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    value["checks"].as_array().unwrap().clone()
}

fn check_named<'a>(checks: &'a [Value], name: &str) -> Option<&'a Value> {
    checks.iter().find(|c| c["name"] == name)
}

#[test]
fn doctor_says_when_it_runs_under_wsl() {
    let sandbox = Sandbox::new();
    let checks = doctor_checks(&sandbox, |command| {
        command.env("WSL_DISTRO_NAME", "Ubuntu");
    });
    let wsl = check_named(&checks, "wsl").expect("a wsl check");
    assert_eq!(wsl["status"], "warn", "{wsl}");
    assert!(
        wsl["detail"]
            .as_str()
            .unwrap()
            .contains("not a supported target")
    );

    let plain = doctor_checks(&sandbox, |command| {
        command.env_remove("WSL_DISTRO_NAME");
    });
    let on_wsl_kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft"));
    if !on_wsl_kernel {
        assert!(check_named(&plain, "wsl").is_none(), "{plain:?}");
    }
}

#[test]
fn doctor_proves_every_grammar_parses() {
    let sandbox = Sandbox::new();
    let checks = doctor_checks(&sandbox, |_| {});
    let grammars = check_named(&checks, "grammars").expect("a grammars check");
    assert_eq!(grammars["status"], "ok", "{grammars}");
    assert!(grammars["detail"].as_str().unwrap().contains("7 languages"));
}

#[test]
fn doctor_reports_a_missing_cache_as_normal_and_a_damaged_one_without_repairing_it() {
    let sandbox = Sandbox::new();
    let checks = doctor_checks(&sandbox, |_| {});
    let cache = check_named(&checks, "cache").unwrap();
    assert_eq!(cache["status"], "ok");
    assert!(cache["detail"].as_str().unwrap().contains("no cache yet"));
    assert!(
        !sandbox.root.path().join("cache/analysis.sqlite3").exists(),
        "asking must not create the cache"
    );

    std::fs::create_dir_all(sandbox.root.path().join("cache")).unwrap();
    let damaged = sandbox.root.path().join("cache/analysis.sqlite3");
    let bytes = b"not a database ".repeat(200);
    std::fs::write(&damaged, &bytes).unwrap();
    let checks = doctor_checks(&sandbox, |_| {});
    let cache = check_named(&checks, "cache").unwrap();
    assert_eq!(cache["status"], "warn", "{cache}");
    assert_eq!(
        std::fs::read(&damaged).unwrap(),
        bytes,
        "doctor must not repair"
    );
}

#[test]
fn over_ssh_doctor_says_so_and_explains_the_clipboard() {
    let sandbox = Sandbox::new();
    let local = doctor_checks(&sandbox, |c| {
        c.env_remove("SSH_CONNECTION")
            .env_remove("SSH_TTY")
            .env_remove("SSH_CLIENT");
    });
    assert!(check_named(&local, "ssh").is_none());

    let remote = doctor_checks(&sandbox, |c| {
        c.env("SSH_CONNECTION", "203.0.113.5 50000 198.51.100.7 22")
            .env("TERM", "xterm-256color")
            .env_remove("TMUX");
    });
    let ssh = check_named(&remote, "ssh").expect("an ssh check over SSH");
    assert_eq!(ssh["status"], "warn");
    let detail = ssh["detail"].as_str().unwrap();
    assert!(
        detail.contains("best effort") && detail.contains("Show file"),
        "{detail}"
    );
    let terminal = check_named(&remote, "terminal").unwrap();
    assert_eq!(terminal["status"], "ok", "a normal TERM over SSH is fine");
}

#[test]
fn a_dumb_terminal_is_a_warning_not_a_failure() {
    let sandbox = Sandbox::new();
    let checks = doctor_checks(&sandbox, |c| {
        c.env("TERM", "dumb");
    });
    assert_eq!(check_named(&checks, "terminal").unwrap()["status"], "warn");
}

#[test]
fn crash_reports_are_listed_and_shown_but_only_from_the_listing() {
    let sandbox = Sandbox::new();
    sandbox
        .klaub()
        .args(["crashes", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No crash reports."));
    sandbox
        .klaub()
        .args(["crashes", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no crash reports"));

    let dir = sandbox.root.path().join("state/crashes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("crash-1800000000-1.txt"),
        "klaub crash report\nmessage:  boom\n",
    )
    .unwrap();
    std::fs::write(sandbox.root.path().join("state/private.txt"), "private").unwrap();

    sandbox
        .klaub()
        .args(["crashes", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("crash-1800000000-1.txt"));
    sandbox
        .klaub()
        .args(["crashes", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("message:  boom"))
        .stderr(predicate::str::contains("before you share"));
    sandbox
        .klaub()
        .args(["crashes", "show", "../private.txt"])
        .assert()
        .failure();

    let checks = doctor_checks(&sandbox, |_| {});
    let crashes = check_named(&checks, "crashes").unwrap();
    assert_eq!(crashes["status"], "warn");
    assert!(
        crashes["detail"]
            .as_str()
            .unwrap()
            .contains("Nothing was sent")
    );
}
