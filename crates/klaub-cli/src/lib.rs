//! # klaub-cli
//!
//! The command line interface of Klaub and its **composition root**: the one place
//! where adapters are chosen and wired to the ports the application needs.
//!
//! Everything lives in a library so the commands can be exercised by tests; the
//! `klaub` binary only calls [`run`].

mod adapters;
mod cli;
mod commands;
mod context;
mod env;
mod logging;
mod output;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

/// Parses the process arguments, runs the command and returns the exit code.
///
/// Errors are printed to standard error as `error: <message>`; usage errors use clap's
/// exit code 2.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match commands::dispatch(cli) {
        Ok(code) => code,
        Err(error) => {
            output::error_line(&format!("{error:#}"));
            ExitCode::FAILURE
        }
    }
}
