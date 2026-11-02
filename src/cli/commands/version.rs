use std::process::ExitCode;

use crate::adapters::tmux::binary::embedded_manifest;
use anyhow::Result;
use serde_json::json;

use crate::domain::i18n::{Locale, cli as t};

use crate::cli::args::Format;
use crate::cli::output;

pub(super) fn run(format: Format, locale: Locale) -> Result<ExitCode> {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let tmux = embedded_manifest();
    match format {
        Format::Json => output::json(&json!({
            "name": "klaub",
            "version": version,
            "os": os,
            "arch": arch,
            "embedded_tmux": tmux.as_ref().map(|m| json!({ "version": m.tmux_version, "target": m.target, "sha256": m.sha256 })),
        }))?,
        Format::Text => {
            output::line(&format!("klaub {version} ({os} {arch})"));
            match tmux {
                Some(m) => output::line(&t::version_embedded_tmux(
                    locale,
                    &m.tmux_version,
                    &m.sha256[..12],
                )),
                None => output::line(t::version_no_embedded_tmux(locale)),
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
