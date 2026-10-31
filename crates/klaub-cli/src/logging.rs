//! Logging setup. Logs never contain code or comments unless the level is raised explicitly.

use std::path::Path;

use tracing_subscriber::EnvFilter;

fn filter() -> Option<EnvFilter> {
    EnvFilter::try_from_env("KLAUB_LOG").ok()
}

/// Logs to standard error at `warn` (or the level in `KLAUB_LOG`).
pub(crate) fn init_stderr() {
    let filter = filter().unwrap_or_else(|| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}

/// Logs to a file, only when `KLAUB_LOG` is set. The terminal belongs to the TUI,
/// so nothing may be written to it.
pub(crate) fn init_file(path: &Path) {
    let Some(filter) = filter() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .with_target(false)
        .try_init();
}
