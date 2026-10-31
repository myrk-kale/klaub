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
    let session = worktree::open_session_of_review(ctx, review)?;
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
    let mut session = worktree::open_session_of_review(ctx, review)?;
    let request = choose(plan(&session), select)?;
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

/// The associated tests of the open review, one request for each distinct test command.
fn plan(session: &ReviewSession) -> Vec<TestRunRequest> {
    let mut groups: Vec<TestRunRequest> = Vec::new();
    for item in &session.analysis().queue {
        let Ok(planned) = session.plan_test_run(&item.unit.id) else {
            continue;
        };
        match groups.iter_mut().find(|g| g.command == planned.command) {
            None => groups.push(planned),
            Some(existing) => {
                for id in planned.selection {
                    if !existing.selection.contains(&id) {
                        existing.selection.push(id);
                    }
                }
            }
        }
    }
    groups
}

/// The one request to run: everything when nothing is selected, otherwise the tests the user
/// named, in the order they named them. The selection is applied first, so a review that spans
/// several test commands can still be narrowed to one with `--select`.
fn choose(mut groups: Vec<TestRunRequest>, select: &[String]) -> Result<TestRunRequest> {
    if !select.is_empty() {
        let mut chosen: Vec<TestId> = Vec::with_capacity(select.len());
        for raw in select {
            let id = TestId::parse(raw).with_context(|| format!("invalid test id {raw:?}"))?;
            if !groups.iter().any(|g| g.selection.contains(&id)) {
                bail!("test {id} is not part of this review's plan");
            }
            if !chosen.contains(&id) {
                chosen.push(id);
            }
        }
        for group in &mut groups {
            group.selection = chosen
                .iter()
                .filter(|id| group.selection.contains(id))
                .cloned()
                .collect();
        }
        groups.retain(|g| !g.selection.is_empty());
    }
    match groups.len() {
        0 => bail!("no association found; there is nothing to run"),
        1 => Ok(groups.remove(0)),
        _ if select.is_empty() => bail!(
            "the review spans more than one test command; pass --select to choose tests for one"
        ),
        _ => bail!("the selected tests need more than one test command; run them separately"),
    }
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

#[cfg(test)]
mod plan_tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;

    fn request(command: &str, tests: &[&str]) -> TestRunRequest {
        TestRunRequest {
            command: vec![command.to_owned()],
            selection: tests.iter().map(|t| TestId::parse(*t).unwrap()).collect(),
            cwd: PathBuf::from("."),
            timeout: Duration::from_secs(1),
        }
    }

    fn names(request: &TestRunRequest) -> Vec<&str> {
        request.selection.iter().map(TestId::as_str).collect()
    }

    #[test]
    fn nothing_selected_runs_the_whole_plan() {
        let chosen = choose(vec![request("pytest", &["a.py::one", "a.py::two"])], &[]).unwrap();
        assert_eq!(names(&chosen), ["a.py::one", "a.py::two"]);
    }

    #[test]
    fn the_selection_is_kept_in_the_order_named_and_without_repeats() {
        let select = [
            "a.py::two".to_owned(),
            "a.py::one".to_owned(),
            "a.py::two".to_owned(),
        ];
        let chosen = choose(
            vec![request("pytest", &["a.py::one", "a.py::two"])],
            &select,
        )
        .unwrap();
        assert_eq!(names(&chosen), ["a.py::two", "a.py::one"]);
    }

    #[test]
    fn a_test_outside_the_plan_is_refused() {
        let error = choose(
            vec![request("pytest", &["a.py::one"])],
            &["a.py::other".to_owned()],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("not part of this review's plan"), "{error}");
    }

    #[test]
    fn two_commands_need_a_selection_and_a_selection_resolves_it() {
        let groups = || {
            vec![
                request("pytest", &["a.py::one"]),
                request("cargo-test", &["b.rs::two"]),
            ]
        };
        let error = choose(groups(), &[]).unwrap_err().to_string();
        assert!(error.contains("pass --select"), "{error}");

        // The hint now works: naming a test narrows the plan to the one command that owns it.
        let chosen = choose(groups(), &["b.rs::two".to_owned()]).unwrap();
        assert_eq!(chosen.command, ["cargo-test"]);
        assert_eq!(names(&chosen), ["b.rs::two"]);

        let both = choose(groups(), &["a.py::one".to_owned(), "b.rs::two".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(both.contains("more than one test command"), "{both}");
    }

    #[test]
    fn an_empty_plan_has_nothing_to_run() {
        let error = choose(Vec::new(), &[]).unwrap_err().to_string();
        assert!(error.contains("nothing to run"), "{error}");
    }
}
