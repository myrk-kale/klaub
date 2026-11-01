//! Command implementations and their dispatch.

mod completions;
mod crashes;
mod doctor;
mod feedback;
mod flow;
mod review;
mod reviews;
mod session;
mod tests;
mod version;
mod worktree;
mod wrap;

use std::process::ExitCode;

use anyhow::Result;

use crate::cli::args::{Cli, Command};
use crate::cli::context::Context;
use crate::cli::logging;

/// Runs the parsed command line.
pub(crate) fn dispatch(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Completions { shell } => Ok(completions::run(shell)),
        Command::Version(args) => version::run(args.format),
        Command::Doctor(args) => doctor::run(cli.tmux_path, args.format),
        Command::Crashes(command) => crashes::run(command),
        Command::Wrap(args) => {
            logging::init_stderr();
            wrap::run(&Context::from_process(cli.tmux_path)?, &args)
        }
        Command::Session(command) => {
            logging::init_stderr();
            session::run(&Context::from_process(cli.tmux_path)?, command)
        }
        Command::Review(args) => review::run(&Context::from_process(cli.tmux_path)?, &args),
        Command::Reviews(command) => reviews::run(&Context::from_process(cli.tmux_path)?, command),
        Command::Feedback(args) => feedback::run(&Context::from_process(cli.tmux_path)?, &args),
        Command::Tests(command) => tests::run(&Context::from_process(cli.tmux_path)?, command),
        Command::Flow(command) => flow::run(&Context::from_process(cli.tmux_path)?, command),
    }
}
