//! Applies input to the reader state. No terminal, no drawing: tests call these functions directly.

use std::time::{Duration, Instant};

use klaub_core::finding::Priority;
use klaub_core::ports::{CancelToken, PrepareOutcome};
use klaub_core::review::{CriterionStatus, Version};
use klaub_core::snapshot::Drift;
use klaub_core::text::{sanitize_single_line, sanitize_terminal_text};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::actions::{
    self, Action, ENTRIES, ensure_code_visible, followup_decision, perform, prepare_confirm,
};
use crate::app::{
    App, Confirm, ConfirmAction, DELIVERY_OPTIONS, Editor, EditorPurpose, FinishStage, Mode,
    Overlay, StatusKind, TreeKind,
};
use crate::viewport::Viewport;

/// How often the worktree is probed for changes.
const PROBE_EVERY: Duration = Duration::from_secs(3);

/// Applies one key press.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind == KeyEventKind::Release {
        return;
    }
    let quit_was_armed = std::mem::take(&mut app.quit_armed);
    if app.overlay.is_some() {
        overlay_key(app, key);
        return;
    }
    app.status = None;
    if mode_key(app, key) {
        return;
    }
    global_key(app, key, quit_was_armed);
}

/// Applies pasted text (terminal bracketed paste) to whichever field is open.
pub(crate) fn handle_paste(app: &mut App, text: &str) {
    match &mut app.overlay {
        Some(Overlay::Editor(editor)) => {
            let clean = if editor.multiline {
                sanitize_terminal_text(text)
            } else {
                sanitize_single_line(text)
            };
            editor.text.push_str(&clean);
        }
        Some(Overlay::Search { input } | Overlay::Palette { input, .. }) => {
            input.push_str(&sanitize_single_line(text));
        }
        _ => {}
    }
}

/// Called when no input arrived for a while: probes the worktree for changes.
pub(crate) fn tick(app: &mut App) {
    if app.last_probe.elapsed() < PROBE_EVERY {
        return;
    }
    app.last_probe = Instant::now();
    let before = app.session.drift();
    match app.session.check_drift() {
        Ok(Drift::Changed) if before != Drift::Changed => app.say(
            StatusKind::Warn,
            "The worktree changed since this snapshot. Press R to take a new one.",
        ),
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, "drift probe failed"),
    }
}

/// Runs an action that blocks, after the frame that announced it was drawn.
pub(crate) fn run_deferred(app: &mut App) {
    let Some(action) = app.deferred.take() else {
        return;
    };
    match action {
        ConfirmAction::RunTests(request) => {
            match app.session.run_tests(&request, &CancelToken::default()) {
                Ok(outcome) => {
                    let what = if outcome.timed_out {
                        "timed out".to_owned()
                    } else if outcome.cancelled {
                        "was cancelled".to_owned()
                    } else {
                        match outcome.exit_code {
                            Some(0) => "passed (exit 0)".to_owned(),
                            Some(code) => format!("failed (exit {code})"),
                            None => "ended without an exit code".to_owned(),
                        }
                    };
                    app.test_output = Some(sanitize_terminal_text(&outcome.output_tail));
                    let kind = if outcome.exit_code == Some(0) {
                        StatusKind::Info
                    } else {
                        StatusKind::Warn
                    };
                    app.say(
                        kind,
                        format!(
                            "Tests {what} in {:.1}s. Reports were re-read.",
                            outcome.duration.as_secs_f32()
                        ),
                    );
                }
                Err(error) => app.say(StatusKind::Error, error.to_string()),
            }
        }
        ConfirmAction::Refresh => match app.session.refresh() {
            Ok(()) => {
                after_refresh(app);
                app.say(
                    StatusKind::Info,
                    "New snapshot taken; comments were relocated.",
                );
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        other => run_confirmed(app, other),
    }
}

fn after_refresh(app: &mut App) {
    let still_there = app
        .unit
        .as_ref()
        .is_some_and(|u| app.session.analysis().units.contains_key(u));
    if !still_there {
        app.unit = app.visible_queue().first().map(|i| i.unit.id.clone());
        app.code = crate::app::CodeState::default();
        if app.mode == Mode::Code {
            app.mode = Mode::Queue;
        }
    } else if app.mode == Mode::Code && app.code.detached.is_none() {
        app.rebuild_code();
    }
    let len = app.visible_queue().len();
    app.queue_view.clamp(len);
    let rows = app.tree_rows().len();
    app.structure_view.clamp(rows);
    app.rebuild_impact();
}

// ---- global keys ---------------------------------------------------------------------------------

fn function_key(n: u8) -> Option<Action> {
    Some(match n {
        1 => Action::Help,
        2 => Action::Mode(Mode::Queue),
        3 => Action::Mode(Mode::Structure),
        4 => Action::Mode(Mode::Code),
        5 => Action::Mode(Mode::Impact),
        6 => Action::Mode(Mode::Comments),
        7 => Action::Mode(Mode::Tests),
        8 => Action::Mode(Mode::Context),
        9 => Action::Mode(Mode::Flow),
        10 => Action::Mode(Mode::Followup),
        _ => return None,
    })
}

fn global_key(app: &mut App, key: KeyEvent, quit_was_armed: bool) {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if control => app.should_quit = true,
        KeyCode::Char('p') if control => {
            app.overlay = Some(Overlay::Palette {
                input: String::new(),
                selected: 0,
            });
        }
        KeyCode::Char(':') => {
            app.overlay = Some(Overlay::Palette {
                input: String::new(),
                selected: 0,
            });
        }
        KeyCode::F(n) => {
            if let Some(action) = function_key(n) {
                perform(app, action);
            }
        }
        KeyCode::Tab => perform(app, Action::Mode(app.mode.cycle(true))),
        KeyCode::BackTab => perform(app, Action::Mode(app.mode.cycle(false))),
        KeyCode::Esc => go_back(app),
        KeyCode::Char('q') if !control => {
            if quit_was_armed {
                app.should_quit = true;
            } else {
                app.quit_armed = true;
                app.say(
                    StatusKind::Info,
                    "Press q again to close the reader. Everything is already saved.",
                );
            }
        }
        KeyCode::Char(c) if !control => {
            if let Some(action) = app.keymap.action(c) {
                perform(app, action);
            }
        }
        _ => {}
    }
}

fn go_back(app: &mut App) {
    if let Some(previous) = app.history.pop() {
        app.mode = previous;
        if previous == Mode::Impact {
            app.rebuild_impact();
        }
    } else if app.filters.any() {
        perform(app, Action::ClearFilters);
    }
}

// ---- lists ------------------------------------------------------------------------------------------

/// Moves a list cursor for navigation keys. Returns whether the key was one.
fn navigate(view: &mut Viewport, key: KeyEvent, len: usize) -> bool {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => view.move_by(1, len),
        KeyCode::Up | KeyCode::Char('k') => view.move_by(-1, len),
        KeyCode::PageDown => view.move_by(view.page(), len),
        KeyCode::PageUp => view.move_by(-view.page(), len),
        KeyCode::Home => view.select(0, len),
        KeyCode::End => view.select(len.saturating_sub(1), len),
        _ => return false,
    }
    true
}

fn is_enter(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Enter)
}

// ---- mode keys -----------------------------------------------------------------------------------------

fn mode_key(app: &mut App, key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }
    match app.mode {
        Mode::Queue => queue_key(app, key),
        Mode::Structure => structure_key(app, key),
        Mode::Code => code_key(app, key),
        Mode::Impact => impact_key(app, key),
        Mode::Comments => comments_key(app, key),
        Mode::Context => context_key(app, key),
        Mode::Tests => scroll_only(&mut app.tests_view, key),
        Mode::Followup => followup_key(app, key),
        Mode::Coverage => coverage_key(app, key),
        Mode::Flow => flow_key(app, key),
    }
}

fn scroll_only(view: &mut Viewport, key: KeyEvent) -> bool {
    navigate(view, key, usize::MAX / 2)
}

fn queue_key(app: &mut App, key: KeyEvent) -> bool {
    let len = app.visible_queue().len();
    if navigate(&mut app.queue_view, key, len) {
        if let Some(unit) = app.queue_selection() {
            app.adopt(unit);
        }
        return true;
    }
    if is_enter(key)
        && let Some(unit) = app.queue_selection()
    {
        app.open_unit(&unit, None);
        return true;
    }
    false
}

fn structure_key(app: &mut App, key: KeyEvent) -> bool {
    let rows = app.tree_rows();
    if navigate(&mut app.structure_view, key, rows.len()) {
        if let Some(row) = rows.get(app.structure_view.selected)
            && let TreeKind::Unit(unit) = &row.kind
        {
            app.adopt(unit.clone());
        }
        return true;
    }
    if !is_enter(key) {
        return false;
    }
    match rows
        .get(app.structure_view.selected)
        .map(|r| r.kind.clone())
    {
        Some(TreeKind::Unit(unit)) => app.open_unit(&unit, None),
        Some(TreeKind::File(path)) => {
            if !app.collapsed.remove(&path) {
                app.collapsed.insert(path);
            }
        }
        Some(TreeKind::Module(module)) => {
            app.filters.module = Some(module.clone());
            app.queue_view = Viewport::default();
            app.go(Mode::Queue);
            app.say(
                StatusKind::Info,
                format!(
                    "Showing module {} only. x clears the filter.",
                    sanitize_single_line(&module)
                ),
            );
        }
        None => {}
    }
    true
}

fn code_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(len) = app.code.view.as_ref().map(|v| v.lines.len()) else {
        return false;
    };
    let height = app.code.height.max(1);
    let page = height.saturating_sub(1).max(1);
    let max = len.saturating_sub(1);
    let selecting = app.code.selection.is_some();
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.code.cursor = (app.code.cursor + 1).min(max),
        KeyCode::Up | KeyCode::Char('k') => app.code.cursor = app.code.cursor.saturating_sub(1),
        KeyCode::PageDown => app.code.cursor = (app.code.cursor + page).min(max),
        KeyCode::PageUp => app.code.cursor = app.code.cursor.saturating_sub(page),
        KeyCode::Home => app.code.cursor = 0,
        KeyCode::End => app.code.cursor = max,
        KeyCode::Right | KeyCode::Char('l') => app.code.hscroll += 8,
        KeyCode::Left | KeyCode::Char('h') => app.code.hscroll = app.code.hscroll.saturating_sub(8),
        KeyCode::Char('V') => {
            app.code.selection = if selecting {
                None
            } else {
                Some(app.code.cursor)
            };
            return true;
        }
        KeyCode::Esc if selecting => {
            app.code.selection = None;
            return true;
        }
        KeyCode::Enter => {
            perform(app, Action::Mode(Mode::Impact));
            return true;
        }
        KeyCode::Char('o') => {
            perform(app, Action::OpenWholeFile);
            return true;
        }
        KeyCode::Char('i') => {
            perform(app, Action::InspectContract);
            return true;
        }
        _ => return false,
    }
    ensure_code_visible(app);
    true
}

fn impact_key(app: &mut App, key: KeyEvent) -> bool {
    let len = app.impact.rows.len();
    if navigate(&mut app.impact_view, key, len) {
        return true;
    }
    let target = app
        .impact
        .rows
        .get(app.impact_view.selected)
        .and_then(|r| r.target.clone());
    match key.code {
        KeyCode::Enter => {
            if let Some(target) = target {
                app.open_symbol(&target);
            }
            true
        }
        KeyCode::Char('p') => {
            match target {
                Some(target) => {
                    app.impact.focus = Some(target);
                    app.rebuild_impact();
                    app.say(
                        StatusKind::Info,
                        "Showing the impact of that symbol. Esc goes back.",
                    );
                }
                None => app.say(
                    StatusKind::Info,
                    "Select a caller, callee or entrypoint to pivot on.",
                ),
            }
            true
        }
        KeyCode::Esc if app.impact.focus.is_some() => {
            app.impact.focus = None;
            app.rebuild_impact();
            true
        }
        _ => false,
    }
}

fn comments_key(app: &mut App, key: KeyEvent) -> bool {
    let len = app.session.review().comments.len();
    if navigate(&mut app.comments_view, key, len) {
        return true;
    }
    let Some(comment) = app
        .session
        .review()
        .comments
        .get(app.comments_view.selected)
        .cloned()
    else {
        return false;
    };
    match key.code {
        KeyCode::Enter => {
            let unit = comment
                .anchor
                .symbol_id
                .clone()
                .filter(|u| app.session.analysis().units.contains_key(u))
                .or_else(|| {
                    app.session
                        .analysis()
                        .queue
                        .iter()
                        .find(|i| i.unit.file == comment.anchor.file_path)
                        .map(|i| i.unit.id.clone())
                });
            match unit {
                Some(unit) => app.open_unit(&unit, Some(comment.anchor.version)),
                None => app.say(
                    StatusKind::Warn,
                    "The code this comment was about is not in this snapshot.",
                ),
            }
            true
        }
        KeyCode::Char('e') => {
            if comment.status == klaub_core::review::CommentStatus::Draft {
                app.overlay = Some(Overlay::Editor(Editor {
                    title: "Edit comment".to_owned(),
                    purpose: EditorPurpose::EditComment(comment.comment_id.clone()),
                    text: comment.text.clone(),
                    severity: comment.severity,
                    multiline: true,
                    error: None,
                }));
            } else {
                app.say(
                    StatusKind::Info,
                    "Only drafts can be edited; this one was already handed over.",
                );
            }
            true
        }
        KeyCode::Char('d') => {
            if comment.status == klaub_core::review::CommentStatus::Draft {
                app.overlay = Some(Overlay::Confirm(Confirm {
                    title: "Delete this comment?".to_owned(),
                    lines: vec![sanitize_single_line(&comment.text)],
                    action: ConfirmAction::DeleteComment(comment.comment_id.clone()),
                }));
            } else {
                app.say(
                    StatusKind::Info,
                    "Only drafts can be deleted; this one was already handed over.",
                );
            }
            true
        }
        _ => false,
    }
}

fn next_criterion_status(status: CriterionStatus) -> CriterionStatus {
    match status {
        CriterionStatus::NotChecked => CriterionStatus::Verified,
        CriterionStatus::Verified => CriterionStatus::Pending,
        CriterionStatus::Pending => CriterionStatus::NotApplicable,
        CriterionStatus::NotApplicable => CriterionStatus::NotChecked,
    }
}

fn context_key(app: &mut App, key: KeyEvent) -> bool {
    // Row 0 is the intent summary; criteria follow.
    let len = 1 + app.session.brief().acceptance_criteria.len();
    if navigate(&mut app.context_view, key, len) {
        return true;
    }
    let edit_intent = |app: &mut App| {
        let text = app
            .session
            .brief()
            .intent_summary
            .clone()
            .unwrap_or_default();
        app.overlay = Some(Overlay::Editor(Editor {
            title: "Intent of the change (what the author wanted)".to_owned(),
            purpose: EditorPurpose::Intent,
            text,
            severity: None,
            multiline: true,
            error: None,
        }));
    };
    match key.code {
        KeyCode::Char('i') => {
            edit_intent(app);
            true
        }
        KeyCode::Char('a') => {
            app.overlay = Some(Overlay::Editor(Editor {
                title: "New acceptance criterion".to_owned(),
                purpose: EditorPurpose::Criterion,
                text: String::new(),
                severity: None,
                multiline: false,
                error: None,
            }));
            true
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            let selected = app.context_view.selected;
            if selected == 0 {
                edit_intent(app);
            } else if let Some(criterion) = app
                .session
                .brief()
                .acceptance_criteria
                .get(selected - 1)
                .cloned()
            {
                let next = next_criterion_status(criterion.status);
                if let Err(error) = app.session.set_criterion(&criterion.id, next, None) {
                    app.say(StatusKind::Error, error.to_string());
                }
            }
            true
        }
        _ => false,
    }
}

fn followup_key(app: &mut App, key: KeyEvent) -> bool {
    let comments: Vec<_> = app.followups().into_iter().cloned().collect();
    if navigate(&mut app.followup_view, key, comments.len()) {
        return true;
    }
    let Some(comment) = comments.get(app.followup_view.selected) else {
        return false;
    };
    if let KeyCode::Char(c) = key.code
        && let Some(status) = followup_decision(c)
    {
        app.overlay = Some(Overlay::Editor(Editor {
            title: format!("Your decision: {} — optional note", status.label()),
            purpose: EditorPurpose::FollowupNote {
                comment: comment.comment_id.clone(),
                status,
            },
            text: String::new(),
            severity: None,
            multiline: false,
            error: None,
        }));
        return true;
    }
    if is_enter(key) {
        let unit = comment
            .anchor
            .symbol_id
            .clone()
            .filter(|u| app.session.analysis().units.contains_key(u));
        match unit {
            Some(unit) => app.open_unit(&unit, Some(Version::After)),
            None => app.say(
                StatusKind::Warn,
                "That code is not in this snapshot; the comment is stale.",
            ),
        }
        return true;
    }
    false
}

fn coverage_key(app: &mut App, key: KeyEvent) -> bool {
    let coverage = app.session.coverage();
    let len = coverage.by_module.len();
    if navigate(&mut app.coverage_view, key, len) {
        return true;
    }
    if is_enter(key)
        && let Some(module) = coverage.by_module.keys().nth(app.coverage_view.selected)
    {
        app.filters.module = Some(module.clone());
        app.filters.unreviewed_only = true;
        app.queue_view = Viewport::default();
        app.go(Mode::Queue);
        app.say(
            StatusKind::Info,
            "Showing what is left to review in that module. x clears the filters.",
        );
        return true;
    }
    false
}

fn flow_key(app: &mut App, key: KeyEvent) -> bool {
    let steps = app
        .session
        .flow()
        .flow
        .as_ref()
        .map_or(0, |f| f.document.steps.len());
    if navigate(&mut app.flow_view, key, steps) {
        return true;
    }
    match key.code {
        KeyCode::Char('p') => {
            perform(app, Action::RequestFlow);
            true
        }
        KeyCode::Char('u') => {
            perform(app, Action::ReloadFlow);
            true
        }
        KeyCode::Enter => {
            let target = app.session.flow().flow.as_ref().and_then(|flow| {
                let step = flow.steps.get(app.flow_view.selected)?;
                let file = step.file.clone()?;
                let symbol = step.symbol.clone()?;
                Some(klaub_core::graph::SymbolKey::new(file, symbol))
            });
            match target {
                Some(key) => app.open_symbol(&key),
                None => app.say(
                    StatusKind::Info,
                    "This step does not point to a symbol in the project.",
                ),
            }
            true
        }
        _ => false,
    }
}

// ---- overlays ----------------------------------------------------------------------------------------------

/// The palette entries that match what was typed.
pub(crate) fn palette_matches(input: &str) -> Vec<&'static actions::Entry> {
    let needle = input.trim().to_lowercase();
    ENTRIES
        .iter()
        .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle))
        .collect()
}

fn overlay_key(app: &mut App, key: KeyEvent) {
    let Some(overlay) = app.overlay.take() else {
        return;
    };
    match overlay {
        Overlay::Explain(unit) => explain_key(app, unit, key),
        Overlay::Help | Overlay::Contract(_) | Overlay::Notice { .. } => {
            // Any key closes; the overlay stays taken.
        }
        Overlay::Palette {
            mut input,
            mut selected,
        } => {
            let matches = palette_matches(&input).len();
            match key.code {
                KeyCode::Esc => return,
                KeyCode::Enter => {
                    let entry = palette_matches(&input).get(selected).copied();
                    if let Some(entry) = entry {
                        perform(app, entry.action);
                    }
                    return;
                }
                KeyCode::Down => selected = (selected + 1).min(matches.saturating_sub(1)),
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Backspace => {
                    input.pop();
                    selected = 0;
                }
                KeyCode::Char(c) if !c.is_control() => {
                    input.push(c);
                    selected = 0;
                }
                _ => {}
            }
            app.overlay = Some(Overlay::Palette { input, selected });
        }
        Overlay::Search { mut input } => match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => {
                input.trim().clone_into(&mut app.filters.search);
                app.queue_view = Viewport::default();
                app.structure_view = Viewport::default();
                let len = app.visible_queue().len();
                if let Some(unit) = app.queue_selection() {
                    app.adopt(unit);
                }
                if app.mode != Mode::Structure {
                    app.mode = Mode::Queue;
                }
                app.say(
                    StatusKind::Info,
                    format!("{len} unit(s) match. x clears the search."),
                );
            }
            KeyCode::Backspace => {
                input.pop();
                app.overlay = Some(Overlay::Search { input });
            }
            KeyCode::Char(c) if !c.is_control() => {
                input.push(c);
                app.overlay = Some(Overlay::Search { input });
            }
            _ => app.overlay = Some(Overlay::Search { input }),
        },
        Overlay::Editor(editor) => editor_key(app, editor, key),
        Overlay::Confirm(confirm) => match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => confirmed(app, confirm.action),
            KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                app.say(StatusKind::Info, "Cancelled. Nothing was changed.");
            }
            _ => app.overlay = Some(Overlay::Confirm(confirm)),
        },
        Overlay::Finish(stage) => finish_key(app, stage, key),
    }
}

/// In "Why this priority", a digit picks a finding and asks to silence its rule for this review.
/// Any other key closes the panel.
fn explain_key(app: &mut App, unit: klaub_core::ids::UnitId, key: KeyEvent) {
    let KeyCode::Char(c @ '1'..='9') = key.code else {
        return;
    };
    let index = usize::from(c as u8 - b'1');
    let Some(finding) = app
        .session
        .analysis()
        .item(&unit)
        .and_then(|item| item.findings.get(index))
    else {
        app.overlay = Some(Overlay::Explain(unit));
        return;
    };
    let rule = finding.rule_id.clone();
    let title = sanitize_single_line(&finding.title);
    app.overlay = Some(Overlay::Confirm(Confirm {
        title: "Silence this rule for this review?".to_owned(),
        lines: vec![
            format!("rule:     {}", sanitize_single_line(&rule)),
            format!("finding:  {title}"),
            "Use it for a false positive. Every finding of this rule disappears from this review"
                .to_owned(),
            "and the scores that counted it drop. The code under review does not change."
                .to_owned(),
            "Bring it back from the palette: \"Bring back silenced rules\".".to_owned(),
        ],
        action: ConfirmAction::MuteRule(rule),
    }));
}

fn editor_key(app: &mut App, mut editor: Editor, key: KeyEvent) {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => return,
        KeyCode::Enter => match save_editor(app, &editor) {
            Ok(()) => return,
            Err(message) => editor.error = Some(message),
        },
        KeyCode::Char('j') if control && editor.multiline => editor.text.push('\n'),
        KeyCode::Tab => {
            if matches!(
                editor.purpose,
                EditorPurpose::NewComment { .. } | EditorPurpose::EditComment(_)
            ) {
                editor.severity = match editor.severity {
                    None => Some(Priority::P0),
                    Some(Priority::P0) => Some(Priority::P1),
                    Some(Priority::P1) => Some(Priority::P2),
                    Some(Priority::P2) => Some(Priority::P3),
                    Some(Priority::P3) => None,
                };
            }
        }
        KeyCode::Backspace => {
            editor.text.pop();
            editor.error = None;
        }
        KeyCode::Char(c) if !control && !c.is_control() => {
            editor.text.push(c);
            editor.error = None;
        }
        _ => {}
    }
    app.overlay = Some(Overlay::Editor(editor));
}

fn save_editor(app: &mut App, editor: &Editor) -> Result<(), String> {
    let text = editor.text.trim();
    let outcome = match &editor.purpose {
        EditorPurpose::NewComment {
            unit,
            version,
            lines,
        } => app
            .session
            .add_comment(unit, *version, *lines, text, editor.severity)
            .map(|_| "Comment saved.".to_owned()),
        EditorPurpose::EditComment(id) => app
            .session
            .edit_comment(id, text, editor.severity)
            .map(|()| "Comment updated.".to_owned()),
        EditorPurpose::Intent => app
            .session
            .set_intent(text)
            .map(|()| "Intent saved.".to_owned()),
        EditorPurpose::Criterion => app
            .session
            .add_criterion(text)
            .map(|_| "Criterion added.".to_owned()),
        EditorPurpose::FollowupNote { comment, status } => {
            let note = (!text.is_empty()).then(|| text.to_owned());
            app.session
                .decide_followup(comment, *status, note)
                .map(|()| format!("Recorded: {}.", status.label()))
        }
    };
    match outcome {
        Ok(message) => {
            app.code.selection = None;
            app.say(StatusKind::Info, message);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn confirmed(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::RunTests(_) => {
            app.say(
                StatusKind::Info,
                "Running tests… the screen is busy until they finish or time out.",
            );
            app.deferred = Some(action);
        }
        ConfirmAction::Refresh => {
            app.say(StatusKind::Info, "Taking a new snapshot…");
            app.deferred = Some(action);
        }
        other => run_confirmed(app, other),
    }
}

fn run_confirmed(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::MuteRule(rule) => match app.session.mute_rule(&rule) {
            Ok(()) => {
                after_refresh(app);
                app.say(
                    StatusKind::Info,
                    format!(
                        "Rule {} is silenced for this review.",
                        sanitize_single_line(&rule)
                    ),
                );
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        ConfirmAction::RestoreRules => match app.session.unmute_all_rules() {
            Ok(()) => {
                after_refresh(app);
                app.say(StatusKind::Info, "Silenced rules are back.");
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        ConfirmAction::PrepareInAgent => {
            let Some(channel) = app.agent.clone() else {
                app.say(
                    StatusKind::Warn,
                    "No agent pane is managed here. Use Copy feedback or Show file.",
                );
                return;
            };
            match app.session.prepare_in_agent(channel.as_ref()) {
                Ok(PrepareOutcome::Prepared) => app.say(
                    StatusKind::Info,
                    "Prepared in the agent's input. Switch to it (prefix + A), read it, and press Enter yourself.",
                ),
                Ok(PrepareOutcome::Unavailable(why)) => app.say(
                    StatusKind::Warn,
                    format!("Could not prepare it: {}. Use Copy feedback or Show file.", sanitize_single_line(&why)),
                ),
                Err(error) => app.say(StatusKind::Error, error.to_string()),
            }
        }
        ConfirmAction::PrepareFlowRequest => {
            let Some(channel) = app.agent.clone() else {
                app.say(
                    StatusKind::Warn,
                    "No agent pane is managed here. Copy the request from the Flow screen instead.",
                );
                return;
            };
            match app.session.prepare_flow_request(channel.as_ref()) {
                Ok(PrepareOutcome::Prepared) => app.say(
                    StatusKind::Info,
                    "Diagram request prepared in the agent. Press Enter there when you want it sent; then U/L reloads the flow.",
                ),
                Ok(PrepareOutcome::Unavailable(why)) => app.say(
                    StatusKind::Warn,
                    format!("Could not prepare it: {}.", sanitize_single_line(&why)),
                ),
                Err(error) => app.say(StatusKind::Error, error.to_string()),
            }
        }
        ConfirmAction::DeleteComment(id) => match app.session.delete_comment(&id) {
            Ok(()) => {
                let len = app.session.review().comments.len();
                app.comments_view.clamp(len);
                app.say(StatusKind::Info, "Comment deleted.");
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        ConfirmAction::RunTests(_) | ConfirmAction::Refresh => confirmed(app, action),
    }
}

fn finish_key(app: &mut App, stage: FinishStage, key: KeyEvent) {
    match stage {
        FinishStage::Confirm { warnings } => match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => match app.session.finish() {
                Ok(record) => {
                    app.overlay = Some(Overlay::Finish(FinishStage::Deliver {
                        record,
                        selected: 0,
                    }));
                }
                Err(error) => app.say(StatusKind::Warn, error.to_string()),
            },
            KeyCode::Esc | KeyCode::Char('n' | 'N') => {}
            _ => app.overlay = Some(Overlay::Finish(FinishStage::Confirm { warnings })),
        },
        FinishStage::Deliver {
            record,
            mut selected,
        } => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(DELIVERY_OPTIONS.len() - 1);
                app.overlay = Some(Overlay::Finish(FinishStage::Deliver { record, selected }));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
                app.overlay = Some(Overlay::Finish(FinishStage::Deliver { record, selected }));
            }
            KeyCode::Esc => {
                app.say(
                    StatusKind::Info,
                    "Review finished and saved. F offers the delivery options again.",
                );
            }
            KeyCode::Enter => deliver(app, &record, selected),
            _ => app.overlay = Some(Overlay::Finish(FinishStage::Deliver { record, selected })),
        },
    }
}

fn deliver(app: &mut App, record: &klaub_core::review::FeedbackRecord, selected: usize) {
    match selected {
        0 => {
            app.overlay = Some(Overlay::Confirm(prepare_confirm(
                app,
                "Prepare the feedback in the agent?",
                &record.prompt,
                ConfirmAction::PrepareInAgent,
            )));
        }
        1 => perform(app, Action::CopyFeedback),
        2 => match app.session.mark_shown() {
            Ok(()) => {
                let file = app.session.review_file();
                app.say(
                    StatusKind::Info,
                    format!(
                        "Feedback file: {file} — tell the agent: {}",
                        sanitize_single_line(&record.prompt)
                    ),
                );
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        _ => app.say(StatusKind::Info, "Review finished and saved."),
    }
}
