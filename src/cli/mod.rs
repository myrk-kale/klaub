//! The command line interface of Klaub and its **composition root**: the one place
//! where adapters are chosen and wired to the ports the application needs.
//!
//! Everything lives in a library so the commands can be exercised by tests; the
//! `klaub` binary only calls [`run`].

mod args;
mod commands;
mod context;
mod crash;
mod env;
mod help;
mod logging;
mod output;

use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches};

use crate::cli::args::Cli;

/// Saves a local report when Klaub panics. Skipped silently when the directories cannot be found.
fn install_crash_reports() {
    let get = |key: &str| std::env::var_os(key);
    let Ok(dirs) = env::resolve_dirs(&get) else {
        return;
    };
    // The subcommand name only: its arguments can be paths.
    let command = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_else(|| "-".to_owned());
    crash::install(dirs.crashes_dir(), env::home(&get), command);
}

/// Parses the process arguments, runs the command and returns the exit code.
///
/// Errors are printed to standard error as `error: <message>`; usage errors use clap's
/// exit code 2.
pub fn run() -> ExitCode {
    let matches = help::localized(Cli::command(), env::process_locale()).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    install_crash_reports();
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(error) => {
            output::error_line(&format!("{error:#}"));
            ExitCode::FAILURE
        }
    }
}
