use std::io::IsTerminal;
use std::process::ExitCode;

use crate::adapters::tmux::session::{LaunchSpec, client_for, launch};
use crate::domain::i18n::cli as t;
use crate::domain::ids::SessionId;
use anyhow::{Result, bail};

use crate::cli::args::WrapArgs;
use crate::cli::context::Context;
use crate::cli::{env, output};

pub(super) fn run(ctx: &Context, args: &WrapArgs) -> Result<ExitCode> {
    if std::env::var_os("TMUX").is_some() {
        bail!("{}", t::wrap_inside_tmux(ctx.locale));
    }

    let cwd = std::env::current_dir()?.canonicalize()?;
    let worktree = env::find_worktree_root(&cwd).unwrap_or_else(|| {
        output::warn_line(t::wrap_not_git(ctx.locale));
        cwd
    });

    let tmux = ctx.resolve_tmux()?;
    let session_id = SessionId::from_digest(&ctx.ids.random_hex());
    let exe = std::env::current_exe()?;
    let review_command = vec![
        exe.to_string_lossy().into_owned(),
        "review".to_owned(),
        "--session".to_owned(),
        session_id.to_string(),
    ];

    let record = launch(
        &ctx.registry(),
        &LaunchSpec {
            session_id: &session_id,
            worktree: &worktree,
            agent_command: &args.command,
            review_command: &review_command,
            klaub_exe: &exe,
            prefix: &args.prefix,
            tmux_binary: &tmux.path,
            created_at: ctx.clock.now(),
        },
    )?;

    if args.detach || !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        let id = record.session_id.to_string();
        output::line(&t::session_started(ctx.locale, &id));
        output::line(&t::session_attach_with(ctx.locale, &id));
        return Ok(ExitCode::SUCCESS);
    }

    // Replaces this process; returns only if tmux could not be started.
    Err(client_for(&record).attach_exec(&record.tmux_session).into())
}
