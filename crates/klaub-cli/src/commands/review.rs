use std::process::ExitCode;

use anyhow::{Context as _, Result};
use klaub_core::ids::SessionId;
use klaub_tui::Welcome;

use crate::cli::ReviewArgs;
use crate::context::Context;
use crate::{env, logging};

pub(super) fn run(ctx: &Context, args: &ReviewArgs) -> Result<ExitCode> {
    logging::init_file(&ctx.dirs.logs_dir().join("review.log"));

    let (session, agent, worktree) = if let Some(raw) = args.session.as_deref() {
        let id = SessionId::parse(raw).context("invalid --session value")?;
        let record = ctx
            .registry()
            .load(&id)
            .with_context(|| format!("cannot load session {id}"))?;
        (
            Some(id.to_string()),
            Some(record.agent_command.join(" ")),
            record.worktree,
        )
    } else {
        let cwd = std::env::current_dir()?;
        (None, None, env::find_worktree_root(&cwd).unwrap_or(cwd))
    };

    let home = ctx.home.as_deref();
    let shown = crate::output::tilde(&worktree, home);
    klaub_tui::run_welcome(&Welcome::new(session.as_deref(), &shown, agent.as_deref()))?;
    Ok(ExitCode::SUCCESS)
}
