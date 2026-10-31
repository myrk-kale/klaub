use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use klaub_core::ids::SessionId;
use klaub_tmux::registry::SessionRecord;
use klaub_tmux::session::{self, FocusTarget as Target, SessionHealth};
use serde_json::json;

use crate::cli::{FocusTarget, Format, SessionCommand};
use crate::context::Context;
use crate::output;

pub(super) fn run(ctx: &Context, command: SessionCommand) -> Result<ExitCode> {
    match command {
        SessionCommand::List(args) => list(ctx, args.format),
        SessionCommand::Attach { id } => attach(ctx, id.as_deref()),
        SessionCommand::Kill { id, yes } => kill(ctx, &id, yes),
        SessionCommand::Focus { id, target } => focus(ctx, &id, target),
        SessionCommand::Reap { id } => reap(ctx, &id),
    }
}

fn parse_id(raw: &str) -> Result<SessionId> {
    let raw = raw.trim();
    let id = if raw.starts_with(SessionId::PREFIX) {
        raw.to_owned()
    } else {
        format!("{}{raw}", SessionId::PREFIX)
    };
    SessionId::parse(id).with_context(|| format!("{raw:?} is not a session id"))
}

fn state_label(health: Option<SessionHealth>) -> &'static str {
    match health {
        Some(SessionHealth::Running) => "running",
        Some(SessionHealth::AgentExited) => "agent-exited",
        Some(SessionHealth::AgentGone) => "agent-gone",
        Some(SessionHealth::ServerDown) => "ended",
        None => "unknown",
    }
}

fn list(ctx: &Context, format: Format) -> Result<ExitCode> {
    let rows: Vec<(SessionRecord, &'static str)> = ctx
        .registry()
        .list()?
        .into_iter()
        .map(|record| {
            let state = state_label(session::health(&record).ok());
            (record, state)
        })
        .collect();

    match format {
        Format::Json => output::json(&json!(
            rows.iter()
                .map(|(record, state)| json!({
                    "id": record.session_id,
                    "state": state,
                    "worktree": record.worktree,
                    "command": record.agent_command,
                    "created_at": record.created_at,
                }))
                .collect::<Vec<_>>()
        ))?,
        Format::Text if rows.is_empty() => output::line("no sessions"),
        Format::Text => {
            output::line(&format!(
                "{:<18} {:<13} {:<32} COMMAND",
                "ID", "STATE", "WORKTREE"
            ));
            for (record, state) in &rows {
                output::line(&format!(
                    "{:<18} {:<13} {:<32} {}",
                    record.session_id,
                    state,
                    output::tilde(&record.worktree, ctx.home.as_deref()),
                    klaub_core::text::sanitize_single_line(&record.agent_command.join(" ")),
                ));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn attach(ctx: &Context, id: Option<&str>) -> Result<ExitCode> {
    let registry = ctx.registry();
    let record = if let Some(raw) = id {
        registry.load(&parse_id(raw)?)?
    } else {
        let mut running: Vec<SessionRecord> = registry
            .list()?
            .into_iter()
            .filter(|r| {
                matches!(
                    session::health(r),
                    Ok(SessionHealth::Running | SessionHealth::AgentExited)
                )
            })
            .collect();
        match running.len() {
            0 => bail!("no running sessions; start one with `klaub wrap <command>`"),
            1 => running.remove(0),
            _ => bail!(
                "several sessions are running ({}); pass one of them to `klaub session attach <id>`",
                running
                    .iter()
                    .map(|r| r.session_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    };
    if session::health(&record)? == SessionHealth::ServerDown {
        bail!(
            "session {} has ended; its record can be removed with `klaub session kill {}`",
            record.session_id,
            record.session_id
        );
    }
    Err(session::client_for(&record)
        .attach_exec(&record.tmux_session)
        .into())
}

fn kill(ctx: &Context, raw: &str, yes: bool) -> Result<ExitCode> {
    let registry = ctx.registry();
    let record = registry.load(&parse_id(raw)?)?;
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to end a session without confirmation; pass --yes");
        }
        let _ = write!(
            std::io::stderr(),
            "End session {} and the agent running in it (`{}`)? [y/N] ",
            record.session_id,
            klaub_core::text::sanitize_single_line(&record.agent_command.join(" "))
        );
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            output::line("cancelled");
            return Ok(ExitCode::FAILURE);
        }
    }
    session::kill(&registry, &record)?;
    output::line(&format!("ended session {}", record.session_id));
    Ok(ExitCode::SUCCESS)
}

fn focus(ctx: &Context, raw: &str, target: FocusTarget) -> Result<ExitCode> {
    let registry = ctx.registry();
    let mut record = registry.load(&parse_id(raw)?)?;
    let exe = std::env::current_exe()?;
    let review_command = vec![
        exe.to_string_lossy().into_owned(),
        "review".to_owned(),
        "--session".to_owned(),
        record.session_id.to_string(),
    ];
    let target = match target {
        FocusTarget::Agent => Target::Agent,
        FocusTarget::Review => Target::Review,
    };
    session::focus(&registry, &mut record, target, &review_command)?;
    Ok(ExitCode::SUCCESS)
}

fn reap(ctx: &Context, raw: &str) -> Result<ExitCode> {
    let registry = ctx.registry();
    // A record that is already gone means there is nothing left to reap.
    let Ok(record) = registry.load(&parse_id(raw)?) else {
        return Ok(ExitCode::SUCCESS);
    };
    session::reap(&registry, &record)?;
    Ok(ExitCode::SUCCESS)
}
