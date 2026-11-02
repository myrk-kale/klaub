//! `klaub crashes`: reading the reports Klaub saved when it panicked.

use std::process::ExitCode;

use anyhow::{Result, bail};
use serde_json::json;

use crate::domain::i18n::cli as t;

use crate::cli::args::{CrashesCommand, Format};
use crate::cli::context::Context;
use crate::cli::{crash, output};

pub(super) fn run(command: CrashesCommand) -> Result<ExitCode> {
    let ctx = Context::from_process(None)?;
    let dir = ctx.dirs.crashes_dir();
    match command {
        CrashesCommand::List(args) => {
            let names = crash::list(&dir);
            match args.format {
                Format::Json => output::json(&json!({ "reports": names }))?,
                Format::Text if names.is_empty() => {
                    output::line(t::crashes_none_listed(ctx.locale))
                }
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
                bail!("{}", t::crashes_none(ctx.locale));
            };
            let Some(text) = crash::read(&dir, &chosen) else {
                bail!("{}", t::crashes_unknown(ctx.locale, &chosen));
            };
            output::error_line(t::crashes_read_first(ctx.locale));
            output::line(text.trim_end());
        }
    }
    Ok(ExitCode::SUCCESS)
}
