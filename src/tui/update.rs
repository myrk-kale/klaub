//! Applies input to the reader state. No terminal, no drawing: tests call these functions directly.

use std::time::{Duration, Instant};

use crate::domain::finding::Priority;
use crate::domain::ports::{CancelToken, PrepareOutcome};
use crate::domain::review::{CriterionStatus, Version};
use crate::domain::snapshot::Drift;
use crate::domain::text::{sanitize_single_line, sanitize_terminal_text};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::tui::actions::{
    self, Action, ENTRIES, ensure_code_visible, followup_decision, perform, prepare_confirm,
};
use crate::tui::app::{
    App, Confirm, ConfirmAction, DELIVERY_OPTIONS, Editor, EditorPurpose, FinishStage, Mode,
    Overlay, Peek, StatusKind, TreeKind,
};
use crate::tui::viewport::Viewport;

/// How often the worktree is probed for changes.
const PROBE_EVERY: Duration = Duration::from_secs(3);

/// Applies one key press.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind == KeyEventKind::Release {
        release(app, key);
        return;
    }
    let key = translated(app, key);
    let quit_was_armed = std::mem::take(&mut app.quit_armed);
    if app.overlay.is_some() {
        overlay_key(app, key);
        return;
    }
    app.status = None;
    if let KeyCode::Char(c) = key.code
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && app.keymap.action(c) == Some(Action::PeekPrevious)
    {
        begin_peek(app, c);
        return;
    }
    if mode_key(app, key) {
        return;
    }
    global_key(app, key, quit_was_armed);
}

// ---- holding a key to see the previous version ---------------------------------------------------

/// Starts looking at the previous version for as long as `key` stays down.
///
/// Holding only works when the terminal reports key releases. Without that the reader could never
/// tell the key was let go, so it says so and leaves `v` as the way to compare.
fn begin_peek(app: &mut App, key: char) {
    if app.peek.is_some() {
        // A held key repeats; the first press already did the work.
        return;
    }
    let toggle = app.keymap.key_for("toggle_version");
    if !app.hold_supported {
        app.say(
            StatusKind::Info,
            format!(
                "This terminal does not report key releases, so {key} cannot be held. {toggle} toggles old and new."
            ),
        );
        return;
    }
    if app.mode != Mode::Code || app.code.view.is_none() {
        app.say(StatusKind::Info, "Open some code first.");
        return;
    }
    if app.code.version == Some(Version::Before) {
        app.say(StatusKind::Info, "Already showing the previous version.");
        return;
    }
    actions::toggle_version(app);
    if app.code.version == Some(Version::Before) {
        app.peek = Some(Peek {
            key,
            restore: Version::After,
        });
    }
}

/// Puts the version back when the held key is let go.
fn release(app: &mut App, key: KeyEvent) {
    if let (Some(peek), KeyCode::Char(c)) = (app.peek, key.code)
        && peek.key == c
    {
        end_peek(app);
    }
}

/// Stops looking at the previous version, whatever ended it: the key going up, the terminal losing
/// focus (the release would never arrive) or the reader being left.
pub fn end_peek(app: &mut App) {
    let Some(peek) = app.peek.take() else {
        return;
    };
    if app.code.view.is_some() && app.code.version != Some(peek.restore) {
        app.code.version = Some(peek.restore);
        app.rebuild_code();
    }
}

// ---- the mouse ----------------------------------------------------------------------------------

/// Lines the wheel moves per notch.
const WHEEL_STEP: usize = 3;

/// Applies one mouse event. The mouse only does what a key already does: the wheel presses
/// `Up`/`Down`, a click on a tab switches to it, and a click on a row selects it (and opens it when
/// it was already selected). Nothing here is reachable by the mouse alone.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollDown => wheel(app, KeyCode::Down),
        MouseEventKind::ScrollUp => wheel(app, KeyCode::Up),
        MouseEventKind::Down(MouseButton::Left) => click(app, mouse.column, mouse.row),
        _ => {}
    }
}

fn plain(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn wheel(app: &mut App, code: KeyCode) {
    if app.overlay.is_some() {
        return;
    }
    for _ in 0..WHEEL_STEP {
        handle_key(app, plain(code));
    }
}

fn inside(area: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

fn click(app: &mut App, x: u16, y: u16) {
    if let Some(overlay) = &app.overlay {
        // The panels that close on any key close on a click too; the rest wait for their keys.
        if matches!(
            overlay,
            Overlay::Help | Overlay::Notice { .. } | Overlay::Contract(_) | Overlay::Explain(_)
        ) {
            app.overlay = None;
        }
        return;
    }
    app.quit_armed = false;
    if let Some(&(_, mode)) = app.tab_hits.iter().find(|(area, _)| inside(*area, x, y)) {
        perform(app, Action::Mode(mode));
        return;
    }
    if app.mode == Mode::Code {
        if inside(app.code.area, x, y)
            && let Some(len) = app.code.view.as_ref().map(|v| v.lines.len())
        {
            let line = app.code.offset + usize::from(y - app.code.area.y);
            app.code.cursor = line.min(len.saturating_sub(1));
            ensure_code_visible(app);
        }
        return;
    }
    let Some(view) = list_of(app) else {
        return;
    };
    let (Some(row), selected) = (view.row_at(x, y), view.selected) else {
        return;
    };
    if row == selected {
        handle_key(app, plain(KeyCode::Enter));
        return;
    }
    // Walk the selection with the same key the keyboard uses, so every side effect of moving
    // (adopting the unit, clamping, scrolling) is the one the keyboard has.
    let code = if row > selected {
        KeyCode::Down
    } else {
        KeyCode::Up
    };
    for _ in 0..row.abs_diff(selected) {
        handle_key(app, plain(code));
    }
}

/// The list the current mode scrolls, if it has one the mouse can pick from.
fn list_of(app: &App) -> Option<&Viewport> {
    Some(match app.mode {
        Mode::Queue => &app.queue_view,
        Mode::Structure => &app.structure_view,
        Mode::Impact => &app.impact_view,
        Mode::Comments => &app.comments_view,
        Mode::Context => &app.context_view,
        Mode::Followup => &app.followup_view,
        Mode::Coverage => &app.coverage_view,
        Mode::Flow => &app.flow_view,
        Mode::Code | Mode::Tests => return None,
    })
}

/// Turns a moved mode-specific key (`[keys] down = "n"`) into the character the mode matches.
///
/// Text fields and the palette read the character as typed, so they are left alone. The delivery
/// list of the finish panel is a list of its own and takes the movement keys.
fn translated(app: &App, mut key: KeyEvent) -> KeyEvent {
    let KeyCode::Char(c) = key.code else {
        return key;
    };
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return key;
    }
    let mode = match &app.overlay {
        None => Some(app.mode),
        Some(Overlay::Finish(FinishStage::Deliver { .. })) => None,
        Some(_) => return key,
    };
    key.code = KeyCode::Char(app.keymap.translate(mode, c));
    key
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
        app.code = crate::tui::app::CodeState::default();
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
            if comment.status == crate::domain::review::CommentStatus::Draft {
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
            if comment.status == crate::domain::review::CommentStatus::Draft {
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
    let rows = app.flow_rows();
    if navigate(&mut app.flow_view, key, rows.len()) {
        return true;
    }
    let selected = rows.get(app.flow_view.selected);
    match key.code {
        KeyCode::Char('p') => {
            perform(app, Action::RequestFlow);
            true
        }
        KeyCode::Char('u') => {
            perform(app, Action::ReloadFlow);
            true
        }
        // Folding hides the branches below a step; the step itself stays in the list.
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => {
            if let Some(line) = selected.filter(|line| line.has_branches) {
                let step = line.row.step;
                let fold = match key.code {
                    KeyCode::Left => true,
                    KeyCode::Right => false,
                    _ => !line.folded,
                };
                if fold {
                    app.flow_folded.insert(step);
                } else {
                    app.flow_folded.remove(&step);
                }
                let len = app.flow_rows().len();
                app.flow_view.clamp(len);
            }
            true
        }
        KeyCode::Enter => {
            let step = selected.map(|line| line.row.step);
            let target = step.and_then(|step| {
                let resolved = app.session.flow().flow.as_ref()?.steps.get(step)?;
                let file = resolved.file.clone()?;
                let symbol = resolved.symbol.clone()?;
                Some(crate::domain::graph::SymbolKey::new(file, symbol))
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
fn explain_key(app: &mut App, unit: crate::domain::ids::UnitId, key: KeyEvent) {
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

fn deliver(app: &mut App, record: &crate::domain::review::FeedbackRecord, selected: usize) {
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
