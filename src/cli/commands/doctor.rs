use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::adapters::tmux::binary::{self, TmuxSource};
use crate::adapters::tmux::session::{self, SessionHealth};
use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::domain::i18n::{Locale, cli as t};

use crate::cli::args::Format;
use crate::cli::context::Context;
use crate::cli::{env, output};

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
    // The JSON report is read by programs, so its sentences stay in one language.
    let l = match format {
        Format::Json => Locale::En,
        Format::Text => env::process_locale(),
    };
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
    checks.push(git_check(l));

    match Context::from_process(tmux_path) {
        Ok(ctx) => {
            checks.push(tmux_check(&ctx, l));
            checks.push(terminal_check(l));
            checks.extend(ssh_check(l));
            checks.extend(wsl_check(l));
            checks.push(clipboard_check(l));
            checks.push(grammars_check(l));
            checks.extend(directory_checks(&ctx, l));
            checks.push(cache_check(&ctx, l));
            checks.push(sessions_check(&ctx, l));
            checks.push(crashes_check(&ctx, l));
        }
        Err(error) => {
            checks.push(check("directories", Status::Fail, error.to_string()));
            checks.push(terminal_check(l));
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

fn git_check(l: Locale) -> Check {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            check("git", Status::Ok, text)
        }
        _ => check("git", Status::Fail, t::doctor_git_missing(l)),
    }
}

fn tmux_check(ctx: &Context, l: Locale) -> Check {
    match ctx.resolve_tmux() {
        Ok(tmux) => {
            let detail = match &tmux.source {
                TmuxSource::Embedded { version, sha256 } => {
                    t::doctor_tmux_embedded(l, version, &sha256[..12])
                }
                TmuxSource::Explicit => {
                    t::doctor_tmux_explicit(l, &output::tilde(&tmux.path, ctx.home.as_deref()))
                }
                TmuxSource::System => {
                    t::doctor_tmux_system(l, &output::tilde(&tmux.path, ctx.home.as_deref()))
                }
            };
            check(
                "tmux",
                Status::Ok,
                t::doctor_tmux_version(l, &detail, &tmux.version),
            )
        }
        Err(error) if ctx.explicit_tmux().is_none() && binary::embedded_manifest().is_none() => {
            check(
                "tmux",
                Status::Warn,
                t::doctor_tmux_optional(l, &error.to_string()),
            )
        }
        Err(error) => check("tmux", Status::Fail, error.to_string()),
    }
}

fn terminal_check(l: Locale) -> Check {
    let unset = || t::doctor_unset(l).to_owned();
    let term = std::env::var("TERM").unwrap_or_else(|_| unset());
    let color = std::env::var("COLORTERM").unwrap_or_else(|_| unset());
    let inside_tmux = std::env::var_os("TMUX").is_some();
    let status = if term == "dumb" || term == t::doctor_unset(l) || inside_tmux {
        Status::Warn
    } else {
        Status::Ok
    };
    let mut detail = format!("TERM={term} COLORTERM={color}");
    if inside_tmux {
        detail.push_str(t::doctor_inside_tmux(l));
    }
    check("terminal", status, detail)
}

/// Notes a session over SSH, where the terminal is remote and the clipboard is not.
///
/// SSH is supported on a best-effort basis: the reader is a terminal
/// program and works through it, but nothing here can reach the clipboard of the machine the user
/// is sitting at.
fn ssh_check(l: Locale) -> Option<Check> {
    let over_ssh = ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT"]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
    over_ssh.then(|| check("ssh", Status::Warn, t::doctor_ssh(l)))
}

/// WSL runs the Linux build, but it is not a supported target yet. The check says so instead of
/// staying silent, and warns about the one thing that is known to hurt: a repository on the
/// Windows file system.
fn wsl_check(l: Locale) -> Option<Check> {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let distro = std::env::var("WSL_DISTRO_NAME").ok();
    if !is_wsl(&release, distro.as_deref()) {
        return None;
    }
    let mut detail = String::from(t::doctor_wsl(l));
    if std::env::current_dir().is_ok_and(|dir| on_windows_mount(&dir)) {
        detail.push_str(t::doctor_wsl_mount(l));
    }
    Some(check("wsl", Status::Warn, detail))
}

/// Whether a kernel release string or a distro name says this is WSL.
fn is_wsl(osrelease: &str, distro: Option<&str>) -> bool {
    let release = osrelease.to_ascii_lowercase();
    distro.is_some_and(|name| !name.is_empty())
        || release.contains("microsoft")
        || release.contains("wsl")
}

/// Whether `dir` is under a Windows drive mounted at `/mnt/<letter>`.
fn on_windows_mount(dir: &Path) -> bool {
    let mut parts = dir.components().filter_map(|c| c.as_os_str().to_str());
    // The root component prints as "/".
    parts.next();
    matches!(
        (parts.next(), parts.next()),
        (Some("mnt"), Some(drive)) if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic())
    )
}

/// Which program `Copy feedback` would use.
fn clipboard_check(l: Locale) -> Check {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["pbcopy"]
    } else {
        &["wl-copy", "xclip"]
    };
    let found = candidates.iter().find(|program| on_path(program));
    match found {
        Some(program) => check(
            "clipboard",
            Status::Ok,
            t::doctor_clipboard_used(l, program),
        ),
        None => check(
            "clipboard",
            Status::Warn,
            t::doctor_clipboard_missing(l, &candidates.join(", ")),
        ),
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Parses a tiny sample in every language to prove each grammar loads and finds a function.
fn grammars_check(l: Locale) -> Check {
    use crate::domain::language::Language;
    use crate::domain::ports::SourceParser;

    let parser = crate::adapters::parse::TreeSitterParser::new();
    let mut broken = Vec::new();
    for language in Language::ALL {
        let sample = match language {
            Language::Python => "def f():\n    pass\n",
            Language::JavaScript => "function f() {}\n",
            Language::TypeScript => "function f(a: number): number { return a }\n",
            Language::Tsx => "const A = () => <div />;\nfunction f() {}\n",
            Language::Rust => "fn f() {}\n",
            Language::Go => "package p\n\nfunc f() {}\n",
            Language::Java => "class P {\n    void f() {}\n}\n",
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
            t::doctor_grammars_ok(l, Language::ALL.len()),
        )
    } else {
        check(
            "grammars",
            Status::Fail,
            t::doctor_grammars_broken(l, &broken.join(", ")),
        )
    }
}

/// The parse cache, looked at without being opened for writing or repaired.
fn cache_check(ctx: &Context, l: Locale) -> Check {
    let path = ctx.dirs.cache_db();
    if !path.exists() {
        return check("cache", Status::Ok, t::doctor_cache_none(l));
    }
    match crate::adapters::storage::SqliteCache::check(&path) {
        Ok(health) if health.is_healthy() => {
            check("cache", Status::Ok, t::doctor_cache_ok(l, health.entries))
        }
        Ok(health) => check(
            "cache",
            Status::Warn,
            t::doctor_cache_degraded(
                l,
                &format!("{:?}", health.integrity),
                &health.schema_version.to_string(),
            ),
        ),
        Err(error) => check(
            "cache",
            Status::Warn,
            t::doctor_cache_unreadable(l, &error.to_string()),
        ),
    }
}

fn crashes_check(ctx: &Context, l: Locale) -> Check {
    let count = crate::cli::crash::list(&ctx.dirs.crashes_dir()).len();
    if count == 0 {
        check("crashes", Status::Ok, t::doctor_crashes_none(l))
    } else {
        check("crashes", Status::Warn, t::doctor_crashes_some(l, count))
    }
}

fn directory_checks(ctx: &Context, l: Locale) -> Vec<Check> {
    let home = ctx.home.as_deref();
    let describe = |path: &Path| {
        let shown = output::tilde(path, home);
        if path.exists() {
            shown
        } else {
            t::doctor_dir_created_on_use(l, &shown)
        }
    };
    vec![
        check("cache dir", Status::Ok, describe(&ctx.dirs.cache)),
        check("state dir", Status::Ok, describe(&ctx.dirs.state)),
        check("config dir", Status::Ok, describe(&ctx.dirs.config)),
    ]
}

fn sessions_check(ctx: &Context, l: Locale) -> Check {
    match ctx.registry().list() {
        Ok(records) => {
            let running = records
                .iter()
                .filter(|r| matches!(session::health(r), Ok(SessionHealth::Running)))
                .count();
            check(
                "sessions",
                Status::Ok,
                t::doctor_sessions(l, running, records.len()),
            )
        }
        Err(error) => check("sessions", Status::Warn, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_is_recognised_by_the_kernel_or_the_distro_name() {
        assert!(is_wsl("5.15.153.1-microsoft-standard-WSL2", None));
        assert!(is_wsl("6.6.0", Some("Ubuntu")));
        assert!(!is_wsl("6.8.0-45-generic", None));
        assert!(!is_wsl("", Some("")));
        assert!(!is_wsl("24.1.0", None), "a macOS kernel is not WSL");
    }

    #[test]
    fn only_a_mounted_windows_drive_counts_as_the_windows_file_system() {
        assert!(on_windows_mount(Path::new("/mnt/c/Users/me/project")));
        assert!(on_windows_mount(Path::new("/mnt/d")));
        assert!(!on_windows_mount(Path::new("/mnt/data/project")));
        assert!(!on_windows_mount(Path::new("/home/me/mnt/c")));
        assert!(!on_windows_mount(Path::new("/mnt")));
    }
}
