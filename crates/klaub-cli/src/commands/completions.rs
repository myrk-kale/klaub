use std::io;
use std::process::ExitCode;

use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;

pub(super) fn run(shell: Shell) -> ExitCode {
    clap_complete::generate(shell, &mut Cli::command(), "klaub", &mut io::stdout());
    ExitCode::SUCCESS
}
