//! `klaub tests`: list the associations of a review and, after confirmation, run them.

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

use crate::app::ReviewSession;
use crate::domain::i18n::{Locale, cli as t};
use crate::domain::ids::TestId;
use crate::domain::ports::{CancelToken, TestRunRequest};
use anyhow::{Context as _, Result, bail};
use serde_json::json;

use crate::cli::args::TestsCommand;
use crate::cli::context::Context;
use crate::cli::output;

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
    let l = ctx.locale;
    let request = choose(plan(&session), select, l)?;
    let preview = ReviewSession::preview_test_run(&request);
    output::line(&preview);
    if !confirmed(yes, l)? {
        output::error_line(t::tests_not_confirmed(l));
        return Ok(ExitCode::FAILURE);
    }
    let outcome = session.run_tests(&request, &CancelToken::default())?;
    if !outcome.output_tail.is_empty() {
        output::line(&outcome.output_tail);
    }
    match outcome.exit_code {
        Some(code) => {
            output::line(&t::tests_exit_code(l, code));
            Ok(exit_code(code))
        }
        None if outcome.timed_out => {
            output::error_line(t::tests_time_limit(l));
            Ok(ExitCode::FAILURE)
        }
        None => {
            output::error_line(t::tests_run_cancelled(l));
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
fn choose(mut groups: Vec<TestRunRequest>, select: &[String], l: Locale) -> Result<TestRunRequest> {
    if !select.is_empty() {
        let mut chosen: Vec<TestId> = Vec::with_capacity(select.len());
        for raw in select {
            let id = TestId::parse(raw).with_context(|| t::tests_invalid_id(l, raw))?;
            if !groups.iter().any(|g| g.selection.contains(&id)) {
                bail!("{}", t::tests_not_in_plan(l, id.as_str()));
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
        0 => bail!("{}", t::tests_nothing_to_run(l)),
        1 => Ok(groups.remove(0)),
        _ if select.is_empty() => bail!("{}", t::tests_several_commands(l)),
        _ => bail!("{}", t::tests_selected_several(l)),
    }
}

/// Whether the user agreed to the preview.
///
/// `--yes` skips the question. Without it, a terminal gets one question; anything else is refused,
/// because a piped `yes` would be an answer the user did not type.
fn confirmed(yes: bool, l: Locale) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        bail!("{}", t::tests_needs_terminal(l));
    }
    let mut stderr = io::stderr().lock();
    write!(stderr, "{}", t::tests_run_prompt(l, t::yes_no_prompt(l)))
        .context(t::tests_cannot_prompt(l))?;
    stderr.flush().context(t::tests_cannot_prompt(l))?;
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .context(t::tests_cannot_read_answer(l))?;
    Ok(output::is_yes(l, &answer))
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
        let chosen = choose(
            vec![request("pytest", &["a.py::one", "a.py::two"])],
            &[],
            Locale::En,
        )
        .unwrap();
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
            Locale::En,
        )
        .unwrap();
        assert_eq!(names(&chosen), ["a.py::two", "a.py::one"]);
    }

    #[test]
    fn a_test_outside_the_plan_is_refused() {
        let error = choose(
            vec![request("pytest", &["a.py::one"])],
            &["a.py::other".to_owned()],
            Locale::En,
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
        let error = choose(groups(), &[], Locale::En).unwrap_err().to_string();
        assert!(error.contains("pass --select"), "{error}");

        // The hint now works: naming a test narrows the plan to the one command that owns it.
        let chosen = choose(groups(), &["b.rs::two".to_owned()], Locale::En).unwrap();
        assert_eq!(chosen.command, ["cargo-test"]);
        assert_eq!(names(&chosen), ["b.rs::two"]);

        let both = choose(
            groups(),
            &["a.py::one".to_owned(), "b.rs::two".to_owned()],
            Locale::En,
        )
        .unwrap_err()
        .to_string();
        assert!(both.contains("more than one test command"), "{both}");
    }

    #[test]
    fn an_empty_plan_has_nothing_to_run() {
        let error = choose(Vec::new(), &[], Locale::En).unwrap_err().to_string();
        assert!(error.contains("nothing to run"), "{error}");
    }
}
