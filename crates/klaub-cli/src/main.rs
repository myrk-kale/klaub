//! `klaub` binary entry point.

use clap::Parser;

/// Klaub: a semantic code reviewer for humans that wraps any coding agent.
#[derive(Debug, Parser)]
#[command(name = "klaub", version, about, long_about = None)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
