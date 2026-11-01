//! `klaub` binary entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    klaub::cli::run()
}
