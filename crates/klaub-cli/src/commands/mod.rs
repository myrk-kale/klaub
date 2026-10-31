//! Command implementations and their dispatch.

mod completions;
mod doctor;
mod review;
mod session;
mod version;
mod wrap;

use std::process::ExitCode;

use anyhow::Result;

use crate::cli::{Cli, Command};
use crate::context::Context;
use crate::logging;

/// Runs the parsed command line.
pub(crate) fn dispatch(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Completions { shell } => Ok(completions::run(shell)),
        Command::Version(args) => version::run(args.format),
        Command::Doctor(args) => doctor::run(cli.tmux_path, args.format),
        Command::Wrap(args) => {
            logging::init_stderr();
            wrap::run(&Context::from_process(cli.tmux_path)?, &args)
        }
        Command::Session(command) => {
            logging::init_stderr();
            session::run(&Context::from_process(cli.tmux_path)?, command)
        }
        Command::Review(args) => review::run(&Context::from_process(cli.tmux_path)?, &args),
    }
}
