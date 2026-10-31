//! `klaub tests`: list the associations of a review and, after confirmation, run them.

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use klaub_app::ReviewSession;
use klaub_core::ids::TestId;
use klaub_core::ports::{CancelToken, TestRunRequest};
use serde_json::json;

use crate::cli::TestsCommand;
use crate::context::Context;
use crate::output;

use super::worktree;

pub(super) fn run(ctx: &Context, command: TestsCommand) -> Result<ExitCode> {
    match command {
        TestsCommand::List { review } => list(ctx, &review),
        TestsCommand::Run {
            review,
            select,
            yes,
        } => run_selected(ctx, &review, &select, yes),
    }
}

fn list(ctx: &Context, review: &str) -> Result<ExitCode> {
    // The flag names the review whose snapshot is open. Associations come from that snapshot;
    // a stored review id is not itself a snapshot, so it is checked and then set aside.
    let _review_id = klaub_core::ids::ReviewId::parse(review).context("invalid review id")?;
    let session = worktree::open_session(ctx)?;
    let rows: Vec<_> = session
        .analysis()
        .queue
        .iter()
        .map(|item| {
            let panel = session.test_panel(&item.unit.id);
            json!({
                "unit": item.unit.id.as_str(),
                "status": panel.status.label(),
                "links": panel.links.iter().map(|link| json!({
                    "test": link.test.id.as_str(),
                    "method": link.method.label(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    output::json(&json!(rows))?;
    Ok(ExitCode::SUCCESS)
}

fn run_selected(ctx: &Context, review: &str, select: &[String], yes: bool) -> Result<ExitCode> {
    let _review_id = klaub_core::ids::ReviewId::parse(review).context("invalid review id")?;
    let mut session = worktree::open_session(ctx)?;
    let mut request = plan(&session)?;
    if !select.is_empty() {
        request.selection = filter_selection(&request, select)?;
    }
    let preview = ReviewSession::preview_test_run(&request);
    output::line(&preview);
    if !confirmed(yes)? {
        output::error_line("not confirmed; nothing was run");
        return Ok(ExitCode::FAILURE);
    }
    let outcome = session.run_tests(&request, &CancelToken::default())?;
    if !outcome.output_tail.is_empty() {
        output::line(&outcome.output_tail);
    }
    match outcome.exit_code {
        Some(code) => {
            output::line(&format!("exit code {code}"));
            Ok(exit_code(code))
        }
        None if outcome.timed_out => {
            output::error_line("the test run hit its time limit");
            Ok(ExitCode::FAILURE)
        }
        None => {
            output::error_line("the test run was cancelled");
            Ok(ExitCode::FAILURE)
        }
    }
}

/// One run covering every associated test of the open review.
fn plan(session: &ReviewSession) -> Result<TestRunRequest> {
    let mut merged: Option<TestRunRequest> = None;
    for item in &session.analysis().queue {
        let Ok(planned) = session.plan_test_run(&item.unit.id) else {
            continue;
        };
        match &mut merged {
            None => merged = Some(planned),
            Some(existing) => {
                if existing.command != planned.command {
                    bail!(
                        "the review spans more than one test command; pass --select to choose one"
                    );
                }
                for id in planned.selection {
                    if !existing.selection.contains(&id) {
                        existing.selection.push(id);
                    }
                }
            }
        }
    }
    merged.context("no association found; there is nothing to run")
}

/// Keeps only the tests the user named, in the order they named them.
fn filter_selection(request: &TestRunRequest, select: &[String]) -> Result<Vec<TestId>> {
    let mut chosen = Vec::with_capacity(select.len());
    for raw in select {
        let id = TestId::parse(raw).with_context(|| format!("invalid test id {raw:?}"))?;
        if !request.selection.contains(&id) {
            bail!("test {id} is not part of this review's plan");
        }
        if !chosen.contains(&id) {
            chosen.push(id);
        }
    }
    Ok(chosen)
}

/// Whether the user agreed to the preview.
///
/// `--yes` skips the question. Without it, a terminal gets one question; anything else is refused,
/// because a piped `yes` would be an answer the user did not type.
fn confirmed(yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        bail!("refusing to run tests without a terminal; pass --yes to confirm");
    }
    let mut stderr = io::stderr().lock();
    write!(stderr, "Run this? [y/N] ").context("cannot prompt for confirmation")?;
    stderr.flush().context("cannot prompt for confirmation")?;
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .context("cannot read the confirmation")?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes" | "YES"))
}

fn exit_code(code: i32) -> ExitCode {
    u8::try_from(code).map_or(ExitCode::FAILURE, ExitCode::from)
}
