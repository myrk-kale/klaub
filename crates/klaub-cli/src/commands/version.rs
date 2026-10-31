use std::process::ExitCode;

use anyhow::Result;
use klaub_tmux::binary::embedded_manifest;
use serde_json::json;

use crate::cli::Format;
use crate::output;

pub(super) fn run(format: Format) -> Result<ExitCode> {
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
                Some(m) => output::line(&format!(
                    "embedded tmux {} (sha256 {})",
                    m.tmux_version,
                    &m.sha256[..12]
                )),
                None => output::line("embedded tmux: none (development build; pass --tmux-path)"),
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
