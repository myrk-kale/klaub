//! `klaub` binary entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    klaub_cli::run()
}
