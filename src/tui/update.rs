//! Applies input to the reader state. No terminal, no drawing: tests call these functions directly.

use std::time::{Duration, Instant};

use crate::domain::i18n::{Locale, tui as t};
use crate::domain::review::Version;
use crate::domain::snapshot::Drift;
use crate::domain::text::sanitize_single_line;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::tui::actions::{self, Action, ENTRIES, ensure_code_visible, perform};
use crate::tui::app::{App, Confirm, ConfirmAction, Mode, Overlay, Peek, StatusKind, TreeKind};
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
            t::peek_unsupported(app.locale, key, toggle),
        );
        return;
    }
    if app.mode != Mode::Code || app.code.view.is_none() {
        app.say(StatusKind::Info, t::open_code_first(app.locale));
        return;
    }
    if app.code.version == Some(Version::Before) {
        app.say(StatusKind::Info, t::peek_already_previous(app.locale));
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
        Mode::Files => &app.files_view,
        Mode::Code => return None,
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
    if app.overlay.is_some() {
        return key;
    }
    key.code = KeyCode::Char(app.keymap.translate(Some(app.mode), c));
    key
}

/// Applies pasted text (terminal bracketed paste) to whichever field is open.
pub(crate) fn handle_paste(app: &mut App, text: &str) {
    if let Some(Overlay::Search { input } | Overlay::Palette { input, .. }) = &mut app.overlay {
        input.push_str(&sanitize_single_line(text));
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
        Ok(Drift::Changed) if before != Drift::Changed => {
            app.say(StatusKind::Warn, t::drift_detected(app.locale));
        }
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
        ConfirmAction::Refresh => match app.session.refresh() {
            Ok(()) => {
                after_refresh(app);
                app.say(StatusKind::Info, t::snapshot_taken(app.locale));
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
    } else if app.mode == Mode::Code {
        app.rebuild_code();
    }
    let len = app.visible_queue().len();
    app.queue_view.clamp(len);
    let rows = app.tree_rows().len();
    app.structure_view.clamp(rows);
}

// ---- global keys ---------------------------------------------------------------------------------

fn function_key(n: u8) -> Option<Action> {
    Some(match n {
        1 => Action::Help,
        2 => Action::Mode(Mode::Queue),
        3 => Action::Mode(Mode::Structure),
        4 => Action::Mode(Mode::Code),
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
                app.say(StatusKind::Info, t::quit_again(app.locale));
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
        Mode::Files => files_key(app, key),
        Mode::Code => code_key(app, key),
    }
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

fn files_key(app: &mut App, key: KeyEvent) -> bool {
    let rows = app.files_rows();
    if navigate(&mut app.files_view, key, rows.len()) {
        if let Some(row) = rows.get(app.files_view.selected) {
            app.adopt(row.first.clone());
        }
        return true;
    }
    if is_enter(key)
        && let Some(row) = rows.get(app.files_view.selected)
    {
        app.open_unit(&row.first.clone(), None);
        return true;
    }
    false
}

fn structure_key(app: &mut App, key: KeyEvent) -> bool {
    let rows = app.tree_rows();
    if navigate(&mut app.structure_view, key, rows.len()) {
        // A file or a module has no code of its own: the code pane shows its first unit.
        let target = rows
            .get(app.structure_view.selected)
            .and_then(|row| match &row.kind {
                TreeKind::Unit(unit) => Some(unit.clone()),
                TreeKind::File(path) => app
                    .visible_queue()
                    .iter()
                    .find(|item| &item.unit.file == path)
                    .map(|item| item.unit.id.clone()),
                TreeKind::Module(module) => app
                    .visible_queue()
                    .iter()
                    .find(|item| {
                        &app.session.analysis().modules.module_of(&item.unit.file) == module
                    })
                    .map(|item| item.unit.id.clone()),
            });
        if let Some(unit) = target {
            app.adopt(unit);
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
                t::module_only(app.locale, &sanitize_single_line(&module)),
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
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.code.cursor = (app.code.cursor + 1).min(max),
        KeyCode::Up | KeyCode::Char('k') => app.code.cursor = app.code.cursor.saturating_sub(1),
        KeyCode::PageDown => app.code.cursor = (app.code.cursor + page).min(max),
        KeyCode::PageUp => app.code.cursor = app.code.cursor.saturating_sub(page),
        KeyCode::Home => app.code.cursor = 0,
        KeyCode::End => app.code.cursor = max,
        KeyCode::Right | KeyCode::Char('l') => app.code.hscroll += 8,
        KeyCode::Left | KeyCode::Char('h') => app.code.hscroll = app.code.hscroll.saturating_sub(8),
        // `Enter` does what it does in the lists: it stays here, the code is already open.
        KeyCode::Enter => return true,
        KeyCode::Char('o') => {
            perform(app, Action::OpenWholeFile);
            return true;
        }
        KeyCode::Char('i') => {
            perform(app, Action::InspectContract);
            return true;
        }
        KeyCode::Char('K') => {
            perform(app, Action::ToggleInspector);
            return true;
        }
        _ => return false,
    }
    ensure_code_visible(app);
    true
}

// ---- overlays ----------------------------------------------------------------------------------------------

/// The palette entries that match what was typed.
pub(crate) fn palette_matches(locale: Locale, input: &str) -> Vec<&'static actions::Entry> {
    let needle = input.trim().to_lowercase();
    ENTRIES
        .iter()
        .filter(|e| needle.is_empty() || e.name(locale).to_lowercase().contains(&needle))
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
            let matches = palette_matches(app.locale, &input).len();
            match key.code {
                KeyCode::Esc => return,
                KeyCode::Enter => {
                    let entry = palette_matches(app.locale, &input).get(selected).copied();
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
                app.say(StatusKind::Info, t::search_matches(app.locale, len));
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
        Overlay::Confirm(confirm) => match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => confirmed(app, confirm.action),
            KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                app.say(StatusKind::Info, t::search_cancelled(app.locale));
            }
            _ => app.overlay = Some(Overlay::Confirm(confirm)),
        },
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
    let l = app.locale;
    app.overlay = Some(Overlay::Confirm(Confirm {
        title: t::mute_title(l).to_owned(),
        lines: vec![
            t::mute_rule_line(l, &sanitize_single_line(&rule)),
            t::mute_finding_line(l, &title),
            t::mute_explain_1(l).to_owned(),
            t::mute_explain_2(l).to_owned(),
            t::mute_restore_hint(l).to_owned(),
        ],
        action: ConfirmAction::MuteRule(rule),
    }));
}

fn confirmed(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::Refresh => {
            app.say(StatusKind::Info, t::snapshot_taking(app.locale));
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
                    t::rule_silenced(app.locale, &sanitize_single_line(&rule)),
                );
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        ConfirmAction::RestoreRules => match app.session.unmute_all_rules() {
            Ok(()) => {
                after_refresh(app);
                app.say(StatusKind::Info, t::rules_restored(app.locale));
            }
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        ConfirmAction::Refresh => confirmed(app, action),
    }
}
