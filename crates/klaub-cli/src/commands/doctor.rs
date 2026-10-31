use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::Result;
use klaub_tmux::binary::{self, TmuxSource};
use klaub_tmux::session::{self, SessionHealth};
use serde::Serialize;
use serde_json::json;

use crate::cli::Format;
use crate::context::Context;
use crate::output;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

fn check(name: &'static str, status: Status, detail: impl Into<String>) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
    }
}

pub(super) fn run(tmux_path: Option<PathBuf>, format: Format) -> Result<ExitCode> {
    let mut checks = vec![check(
        "klaub",
        Status::Ok,
        format!(
            "{} ({} {})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    )];
    checks.push(git_check());

    match Context::from_process(tmux_path) {
        Ok(ctx) => {
            checks.push(tmux_check(&ctx));
            checks.push(terminal_check());
            checks.extend(ssh_check());
            checks.push(clipboard_check());
            checks.push(grammars_check());
            checks.extend(directory_checks(&ctx));
            checks.push(cache_check(&ctx));
            checks.push(sessions_check(&ctx));
            checks.push(crashes_check(&ctx));
        }
        Err(error) => {
            checks.push(check("directories", Status::Fail, error.to_string()));
            checks.push(terminal_check());
        }
    }

    let failed = checks.iter().any(|c| c.status == Status::Fail);
    match format {
        Format::Json => output::json(&json!({ "ok": !failed, "checks": checks }))?,
        Format::Text => {
            for c in &checks {
                let mark = match c.status {
                    Status::Ok => "ok  ",
                    Status::Warn => "warn",
                    Status::Fail => "FAIL",
                };
                output::line(&format!("[{mark}] {:<12} {}", c.name, c.detail));
            }
        }
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn git_check() -> Check {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            check("git", Status::Ok, text)
        }
        _ => check(
            "git",
            Status::Fail,
            "`git` was not found in PATH; Klaub drives the git executable",
        ),
    }
}

fn tmux_check(ctx: &Context) -> Check {
    match ctx.resolve_tmux() {
        Ok(tmux) => {
            let detail = match &tmux.source {
                TmuxSource::Embedded { version, sha256 } => {
                    format!("embedded {version}, sha256 {} verified", &sha256[..12])
                }
                TmuxSource::Explicit => format!(
                    "{} (explicit path, not verified)",
                    output::tilde(&tmux.path, ctx.home.as_deref())
                ),
            };
            check(
                "tmux",
                Status::Ok,
                format!("{detail}; version {}", tmux.version),
            )
        }
        Err(error) if ctx.explicit_tmux().is_none() && binary::embedded_manifest().is_none() => {
            check(
                "tmux",
                Status::Warn,
                format!("{error}. `klaub review` works without it"),
            )
        }
        Err(error) => check("tmux", Status::Fail, error.to_string()),
    }
}

fn terminal_check() -> Check {
    let term = std::env::var("TERM").unwrap_or_else(|_| "unset".to_owned());
    let color = std::env::var("COLORTERM").unwrap_or_else(|_| "unset".to_owned());
    let inside_tmux = std::env::var_os("TMUX").is_some();
    let status = if term == "dumb" || term == "unset" || inside_tmux {
        Status::Warn
    } else {
        Status::Ok
    };
    let mut detail = format!("TERM={term} COLORTERM={color}");
    if inside_tmux {
        detail.push_str("; inside tmux: `klaub wrap` is unavailable here, `klaub review` works");
    }
    check("terminal", status, detail)
}

/// Notes a session over SSH, where the terminal is remote and the clipboard is not.
///
/// SSH is supported on a best-effort basis (the specification says so): the reader is a terminal
/// program and works through it, but nothing here can reach the clipboard of the machine the user
/// is sitting at.
fn ssh_check() -> Option<Check> {
    let over_ssh = ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT"]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
    over_ssh.then(|| {
        check(
            "ssh",
            Status::Warn,
            "this is an SSH session: best effort. Copy feedback goes to the clipboard of this host, \
             not of your machine, so use Show file (or `klaub feedback`) to read it",
        )
    })
}

/// Which program `Copy feedback` would use.
fn clipboard_check() -> Check {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["pbcopy"]
    } else {
        &["wl-copy", "xclip"]
    };
    let found = candidates.iter().find(|program| on_path(program));
    match found {
        Some(program) => check("clipboard", Status::Ok, format!("`{program}` will be used")),
        None => check(
            "clipboard",
            Status::Warn,
            format!(
                "none of {} was found; Copy feedback will say so and Show file still works",
                candidates.join(", ")
            ),
        ),
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Parses a tiny sample in every language to prove each grammar loads and finds a function.
fn grammars_check() -> Check {
    use klaub_core::language::Language;
    use klaub_core::ports::SourceParser;

    let parser = klaub_parse::TreeSitterParser::new();
    let mut broken = Vec::new();
    for language in Language::ALL {
        let sample = match language {
            Language::Python => "def f():\n    pass\n",
            Language::JavaScript => "function f() {}\n",
            Language::TypeScript => "function f(a: number): number { return a }\n",
            Language::Tsx => "const A = () => <div />;\nfunction f() {}\n",
            Language::Rust => "fn f() {}\n",
            Language::Go => "package p\n\nfunc f() {}\n",
        };
        // The module symbol always exists; a working grammar also finds the function.
        let works = parser
            .parse(language, sample)
            .is_ok_and(|parsed| parsed.symbols.len() > 1);
        if !works {
            broken.push(language.name());
        }
    }
    if broken.is_empty() {
        check(
            "grammars",
            Status::Ok,
            format!("{} languages parse a sample", Language::ALL.len()),
        )
    } else {
        check(
            "grammars",
            Status::Fail,
            format!("did not parse a sample: {}", broken.join(", ")),
        )
    }
}

/// The parse cache, looked at without being opened for writing or repaired.
fn cache_check(ctx: &Context) -> Check {
    let path = ctx.dirs.cache_db();
    if !path.exists() {
        return check(
            "cache",
            Status::Ok,
            "no cache yet (created by the first review)",
        );
    }
    match klaub_storage::SqliteCache::check(&path) {
        Ok(health) if health.is_healthy() => check(
            "cache",
            Status::Ok,
            format!("{} entries, integrity ok", health.entries),
        ),
        Ok(health) => check(
            "cache",
            Status::Warn,
            format!(
                "integrity {:?}, schema {}: the next review rebuilds it, nothing is lost",
                health.integrity, health.schema_version
            ),
        ),
        Err(error) => check(
            "cache",
            Status::Warn,
            format!("unreadable ({error}): the next review replaces it, nothing is lost"),
        ),
    }
}

fn crashes_check(ctx: &Context) -> Check {
    let count = crate::crash::list(&ctx.dirs.crashes_dir()).len();
    if count == 0 {
        check("crashes", Status::Ok, "no crash reports")
    } else {
        check(
            "crashes",
            Status::Warn,
            format!(
                "{count} saved on this machine; `klaub crashes list` shows them. Nothing was sent"
            ),
        )
    }
}

fn directory_checks(ctx: &Context) -> Vec<Check> {
    let home = ctx.home.as_deref();
    let describe = |path: &Path| {
        let shown = output::tilde(path, home);
        if path.exists() {
            shown
        } else {
            format!("{shown} (created on first use)")
        }
    };
    vec![
        check("cache dir", Status::Ok, describe(&ctx.dirs.cache)),
        check("state dir", Status::Ok, describe(&ctx.dirs.state)),
        check("config dir", Status::Ok, describe(&ctx.dirs.config)),
    ]
}

fn sessions_check(ctx: &Context) -> Check {
    match ctx.registry().list() {
        Ok(records) => {
            let running = records
                .iter()
                .filter(|r| matches!(session::health(r), Ok(SessionHealth::Running)))
                .count();
            check(
                "sessions",
                Status::Ok,
                format!("{running} running, {} recorded", records.len()),
            )
        }
        Err(error) => check("sessions", Status::Warn, error.to_string()),
    }
}
