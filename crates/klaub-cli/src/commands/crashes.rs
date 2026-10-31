//! `klaub crashes`: reading the reports Klaub saved when it panicked.

use std::process::ExitCode;

use anyhow::{Result, bail};
use serde_json::json;

use crate::cli::{CrashesCommand, Format};
use crate::context::Context;
use crate::{crash, output};

pub(super) fn run(command: CrashesCommand) -> Result<ExitCode> {
    let ctx = Context::from_process(None)?;
    let dir = ctx.dirs.crashes_dir();
    match command {
        CrashesCommand::List(args) => {
            let names = crash::list(&dir);
            match args.format {
                Format::Json => output::json(&json!({ "reports": names }))?,
                Format::Text if names.is_empty() => output::line("No crash reports."),
                Format::Text => {
                    for name in &names {
                        output::line(name);
                    }
                }
            }
        }
        CrashesCommand::Show { name } => {
            let names = crash::list(&dir);
            let Some(chosen) = name.or_else(|| names.last().cloned()) else {
                bail!("there are no crash reports");
            };
            let Some(text) = crash::read(&dir, &chosen) else {
                bail!("there is no crash report called {chosen}; `klaub crashes list` shows them");
            };
            output::error_line(
                "Read this before you share it: it holds the panic message, which can quote the data that caused it.",
            );
            output::line(text.trim_end());
        }
    }
    Ok(ExitCode::SUCCESS)
}
