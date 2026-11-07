//! Draws one frame of the reader.
//!
//! The layout, top to bottom: a title bar, a row of mode tabs, an optional banner when the
//! worktree drifted, the mode's own view in a rounded panel, and a status bar. Overlays (help, the
//! palette, the editor, a confirmation) are drawn last, centred over the mode view, with the rest
//! of the screen dimmed behind them.
//!
//! Nothing drawn here comes from the repository, a comment or a document without passing through
//! [`clean`] first, so a hostile sequence in a symbol name cannot reach the terminal.

use crate::app::{ContractDelta, Delta, FileContract};
use crate::domain::finding::Priority;
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::review::FeedbackRecord;
use crate::domain::snapshot::Drift;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::tui::actions::ENTRIES;
use crate::tui::app::{
    App, Confirm, Editor, EditorPurpose, FinishStage, Mode, Overlay, StatusKind, delivery_options,
};
use crate::tui::keymap::Keymap;
use crate::tui::labels;
use crate::tui::theme::{Chip, Theme};
use crate::tui::update::palette_matches;
use crate::tui::views::{draw_mode, explain_lines};
use crate::tui::widgets::{chip_span, clean, dialog, fill_line, fit, gauge, wrap};

/// Below this the reader cannot lay out its bars, so it says so instead of drawing a broken frame.
const MIN_WIDTH: u16 = 50;
const MIN_HEIGHT: u16 = 10;

/// Draws the whole screen into `frame`.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area, app.locale, app.theme);
        return;
    }

    let theme = app.theme;
    app.tab_hits.clear();
    let drift = app.session.drift() == Drift::Changed;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(u16::from(drift)),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let (title, tabs, banner, outer, status) = (rows[0], rows[1], rows[2], rows[3], rows[4]);
    // Breathing room: a row under the tabs, one above the status bar, two columns at each side.
    let body = content_area(outer);

    draw_title(frame, margin(title), app, theme);
    app.tab_hits = draw_tabs(
        frame,
        margin(tabs),
        app.mode,
        app.list,
        app.locale,
        &app.keymap,
        theme,
    );
    if drift {
        draw_banner(frame, margin(banner), app.locale, theme);
    }
    draw_mode(frame, body, app);
    draw_status(frame, margin(status), app, theme);

    if let Some(overlay) = app.overlay.clone() {
        if !theme.is_high_contrast() {
            frame
                .buffer_mut()
                .set_style(area, Style::default().add_modifier(Modifier::DIM));
        }
        draw_overlay(frame, outer, app, &overlay, theme);
    }
}

/// Columns kept empty at each side of the screen.
const MARGIN: u16 = 2;

/// `area` without the side margins.
fn margin(area: Rect) -> Rect {
    let side = MARGIN.min(area.width / 2);
    Rect {
        x: area.x + side,
        width: area.width - side * 2,
        ..area
    }
}

/// The room for a mode's view: side margins, a blank row above and another below. Short
/// terminals keep every row, because the margin is only a courtesy.
fn content_area(outer: Rect) -> Rect {
    let area = margin(outer);
    if area.height < 12 {
        return area;
    }
    Rect {
        y: area.y + 1,
        height: area.height - 2,
        ..area
    }
}

fn draw_too_small(frame: &mut Frame<'_>, area: Rect, locale: Locale, theme: Theme) {
    let message = t::too_small(locale, area.width, area.height, MIN_WIDTH, MIN_HEIGHT);
    let lines: Vec<Line<'static>> = wrap(&message, usize::from(area.width).max(1))
        .into_iter()
        .map(|line| Line::from(Span::styled(line, theme.warn())))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

// ---- title bar --------------------------------------------------------------------------------------------------

fn draw_title(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let snapshot = &app.session.analysis().snapshot;
    let repo = snapshot.worktree.root.file_name().map_or_else(
        || snapshot.worktree.root.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let scope = snapshot.scope.label();
    let id = short_id(snapshot.id.as_str());
    let coverage = app.session.coverage();
    let progress = format!("{}/{}", coverage.reviewed, coverage.total);
    let state = labels::review_status(app.locale, app.session.review().status);

    let width = usize::from(area.width);
    let brand = " ◆ klaub ";
    let right_text = t::title_progress(app.locale, &progress, state);
    let (filled, track) = if width >= 70 {
        gauge(coverage.reviewed, coverage.total, 8)
    } else {
        (String::new(), String::new())
    };
    let gauge_w = filled.width() + track.width();
    let right_w = right_text.width() + gauge_w;
    let left = format!(" {} · {scope} · {id}", clean(&repo));
    let left = fit(&left, width.saturating_sub(brand.width() + right_w));
    let spacer = width.saturating_sub(brand.width() + left.width() + right_w);

    let percent = crate::tui::widgets::percent(coverage.reviewed, coverage.total);
    let line = Line::from(vec![
        Span::styled(brand, theme.brand()),
        Span::styled(left, theme.bar()),
        Span::styled(" ".repeat(spacer), theme.bar()),
        Span::styled(filled, theme.on_bar(theme.gauge(percent))),
        Span::styled(track, theme.on_bar(theme.faint())),
        Span::styled(right_text, theme.bar().add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(line).style(theme.bar()), area);
}

/// The snapshot id without its `snap_` prefix, kept short for the bar.
fn short_id(id: &str) -> &str {
    let bare = id.strip_prefix("snap_").unwrap_or(id);
    let end = bare.char_indices().nth(8).map_or(bare.len(), |(i, _)| i);
    &bare[..end]
}

// ---- mode tabs --------------------------------------------------------------------------------------------------

/// The id a mode's key is filed under in the key map.
fn mode_key_id(mode: Mode) -> &'static str {
    match mode {
        Mode::Queue => "queue",
        Mode::Structure => "structure",
        Mode::Files => "files",
        Mode::Code => "code",
        Mode::Impact => "impact",
        Mode::Comments => "comments",
        Mode::Context => "context",
        Mode::Tests => "tests",
        Mode::Followup => "followup",
        Mode::Coverage => "coverage",
        Mode::Flow => "flow",
    }
}

/// The modes that share the review workspace: a list on the left and the code on the right.
fn in_workspace(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Queue | Mode::Structure | Mode::Files | Mode::Code
    )
}

/// Draws the row of mode tabs and returns where each one is, for the mouse. The four workspace
/// modes are one tab, `Review`; the lists inside it are picked in the pane's own header.
fn draw_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    current: Mode,
    list: Mode,
    locale: Locale,
    keymap: &Keymap,
    theme: Theme,
) -> Vec<(Rect, Mode)> {
    let width = usize::from(area.width);
    let mut tabs: Vec<(bool, Mode, String, String)> = vec![(
        in_workspace(current),
        list,
        format!(" {}-{} ", keymap.key_for("queue"), keymap.key_for("files")),
        format!("{} ", t::tab_review(locale)),
    )];
    for mode in Mode::ALL.iter().filter(|m| !in_workspace(**m)) {
        tabs.push((
            *mode == current,
            *mode,
            format!(" {} ", keymap.key_for(mode_key_id(*mode))),
            format!("{} ", mode.label(locale)),
        ));
    }
    let full: usize = tabs.iter().map(|(_, _, n, m)| n.width() + m.width()).sum();

    // A narrow terminal shows only the current mode rather than a row of clipped names.
    if full > width {
        let label = format!(" {} ", current.label(locale));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                fit(&label, width),
                theme.tab_active(),
            ))),
            area,
        );
        return Vec::new();
    }

    let mut hits = Vec::with_capacity(tabs.len());
    let mut x = area.x;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (active, mode, number, name) in tabs {
        let cells = u16::try_from(number.width() + name.width()).unwrap_or(u16::MAX);
        hits.push((Rect::new(x, area.y, cells, 1), mode));
        x = x.saturating_add(cells);
        if active {
            spans.push(Span::styled(format!("{number}{name}"), theme.tab_active()));
        } else {
            // The number is quieter than the name, so the row reads as names first.
            spans.push(Span::styled(number, theme.faint()));
            spans.push(Span::styled(name, theme.tab_idle()));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    hits
}

fn draw_banner(frame: &mut Frame<'_>, area: Rect, locale: Locale, theme: Theme) {
    let text = t::drift_banner(locale);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            chip_span("!", theme.chip(Chip::Warn)),
            Span::styled(
                fit(text, usize::from(area.width).saturating_sub(3)),
                theme.warn(),
            ),
        ])),
        area,
    );
}

// ---- status bar -------------------------------------------------------------------------------------------------

fn draw_status(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let width = usize::from(area.width);
    let mode = format!(" {} ", app.mode.label(app.locale).to_uppercase());
    let mut spans = vec![
        Span::styled(mode.clone(), theme.brand()),
        Span::styled(" ", theme.bar()),
    ];
    let mut room = width.saturating_sub(mode.width() + 1);

    match &app.status {
        Some((text, kind)) => {
            let (icon, style) = match kind {
                StatusKind::Info => ("●", theme.accent()),
                StatusKind::Warn => ("▲", theme.warn()),
                StatusKind::Error => ("✖", theme.error()),
            };
            spans.push(Span::styled(format!("{icon} "), theme.on_bar(style)));
            let text_style = match kind {
                StatusKind::Info => theme.bar(),
                StatusKind::Warn => theme.on_bar(theme.warn()),
                StatusKind::Error => theme.on_bar(theme.error()),
            };
            spans.push(Span::styled(
                fit(&clean(text), room.saturating_sub(2)),
                text_style,
            ));
        }
        None if app.session.analysis().config.ui.show_hints => {
            let sep = " · ";
            for (index, (key, what)) in
                hints_for(app.locale, app.mode, app.filters.any(), &app.keymap)
                    .into_iter()
                    .enumerate()
            {
                let need = key.width() + 1 + what.width() + if index > 0 { sep.len() } else { 0 };
                if need > room {
                    break;
                }
                room -= need;
                if index > 0 {
                    spans.push(Span::styled(sep, theme.on_bar(theme.faint())));
                }
                spans.push(Span::styled(key, theme.on_bar(theme.key())));
                spans.push(Span::styled(format!(" {what}"), theme.bar_dim()));
            }
        }
        None => {}
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.bar()), area);
}

/// The keys worth knowing in the current mode, as `(key, what it does)`, written with the keys the
/// reader is really using. Shown only while no status message is pending.
fn hints_for(l: Locale, mode: Mode, filtered: bool, keys: &Keymap) -> Vec<(String, String)> {
    let k = |id: &str| keys.key_for(id);
    let hint = |key: &dyn ToString, what: &str| (key.to_string(), what.to_owned());
    let moves = format!("{}/{}", k("down"), k("up"));
    match mode {
        Mode::Queue => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open(l)),
            hint(&k("comment"), t::hint_comment(l)),
            hint(&k("toggle_reviewed"), t::hint_reviewed(l)),
            hint(&k("explain"), t::hint_why(l)),
            hint(&k("search"), t::hint_search(l)),
            if filtered {
                hint(&k("clear_filters"), t::hint_clear_filters(l))
            } else {
                hint(&k("help"), t::hint_help(l))
            },
            hint(&"q q".to_owned(), t::hint_quit(l)),
        ],
        Mode::Structure => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open(l)),
            hint(
                &format!("{}/{}", k("deeper_tree"), k("shallower_tree")),
                t::hint_depth(l),
            ),
            hint(&k("comment"), t::hint_comment(l)),
            hint(&k("toggle_reviewed"), t::hint_reviewed(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Files => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open(l)),
            hint(&k("comment"), t::hint_comment(l)),
            hint(&k("toggle_reviewed"), t::hint_reviewed(l)),
            hint(&k("search"), t::hint_search(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Code => vec![
            hint(&moves, t::hint_move(l)),
            hint(&k("inspect_contract"), t::hint_contract(l)),
            hint(&k("open_file"), t::hint_file(l)),
            hint(&k("toggle_version"), t::hint_old_new(l)),
            hint(
                &format!("{} {}", k("previous_change"), k("next_change")),
                t::hint_change(l),
            ),
            hint(&k("comment"), t::hint_comment(l)),
            hint(&k("select_lines"), t::hint_select(l)),
            hint(
                &format!("{} {}", k("history_back"), k("history_forward")),
                t::hint_back_forward(l),
            ),
        ],
        Mode::Impact => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open_symbol(l)),
            hint(&k("impact"), t::hint_back_to_queue(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Comments => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open_code(l)),
            hint(&k("edit_comment"), t::hint_edit(l)),
            hint(&k("delete_comment"), t::hint_delete_draft(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Context => vec![
            hint(&moves, t::hint_scroll(l)),
            hint(&k("edit_intent"), t::hint_set_intent(l)),
            hint(&k("add_criterion"), t::hint_add_criterion(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Tests => vec![
            hint(&moves, t::hint_move(l)),
            hint(&k("run_tests"), t::hint_run_tests(l)),
            hint(&k("reload_reports"), t::hint_reload_reports(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Followup => vec![
            hint(&moves, t::hint_move(l)),
            hint(&k("decide_resolved"), t::hint_resolved(l)),
            hint(&k("decide_partial"), t::hint_partial(l)),
            hint(&k("decide_unresolved"), t::hint_not_resolved(l)),
            hint(&k("decide_regression"), t::hint_regression(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Coverage => vec![
            hint(&moves, t::hint_move(l)),
            hint(&"Enter".to_owned(), t::hint_open_pending(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
        Mode::Flow => vec![
            hint(&moves, t::hint_scroll(l)),
            hint(&k("request_flow"), t::hint_ask_diagram(l)),
            hint(&k("reload_flow"), t::hint_reload(l)),
            hint(&k("help"), t::hint_help(l)),
        ],
    }
}

// ---- overlays ---------------------------------------------------------------------------------------------------

fn draw_overlay(frame: &mut Frame<'_>, body: Rect, app: &App, overlay: &Overlay, theme: Theme) {
    match overlay {
        Overlay::Help => draw_help(frame, body, app, theme),
        Overlay::Palette { input, selected } => {
            draw_palette(frame, body, app, input, *selected, theme);
        }
        Overlay::Search { input } => draw_search(frame, body, input, app.locale, theme),
        Overlay::Editor(editor) => draw_editor(frame, body, editor, app.locale, theme),
        Overlay::Confirm(confirm) => draw_confirm(frame, body, confirm, app.locale, theme),
        Overlay::Notice { title, text } => {
            draw_notice(frame, body, title, text, app.locale, theme);
        }
        Overlay::Explain(unit) => draw_explain(frame, body, app, unit, theme),
        Overlay::Contract(unit) => draw_contract(frame, body, app, unit, theme),
        Overlay::Finish(stage) => draw_finish(frame, body, stage, app.locale, theme),
    }
}

/// A panel centred in `body`, never larger than it, with a titled border.
fn panel(body: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(body.width);
    let height = height.min(body.height);
    Rect {
        x: body.x + (body.width - width) / 2,
        y: body.y + (body.height - height) / 2,
        width,
        height,
    }
}

/// The width of the text inside an overlay: the border and one column of padding on each side.
fn text_width(area: Rect) -> usize {
    usize::from(area.width).saturating_sub(4)
}

/// `key  what` pairs on one line, the keys in the key colour.
fn hint_line(parts: &[(&str, &str)], theme: Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (key, what) in parts {
        if !key.is_empty() {
            spans.push(Span::styled((*key).to_owned(), theme.key()));
        }
        spans.push(Span::styled((*what).to_owned(), theme.dim()));
    }
    Line::from(spans)
}

/// A row of a chooser: the chosen one is filled across the whole row.
fn choice_row(text: &str, chosen: bool, width: usize, theme: Theme) -> Line<'static> {
    if chosen {
        let bar = if theme.is_high_contrast() {
            "▶"
        } else {
            "▌"
        };
        fill_line(
            Line::from(vec![
                Span::styled(bar, theme.accent()),
                Span::raw(format!(" {text}")),
            ])
            .style(theme.selected()),
            width,
        )
    } else {
        Line::from(Span::raw(format!("  {text}")))
    }
}

fn draw_help(frame: &mut Frame<'_>, body: Rect, app: &App, theme: Theme) {
    let l = app.locale;
    let area = panel(
        body,
        body.width.saturating_sub(4),
        body.height.saturating_sub(2),
    );
    let inner = text_width(area);
    let mut lines = vec![Line::from(Span::styled(
        fit(t::help_close(l), inner),
        theme.dim(),
    ))];
    for entry in ENTRIES {
        let label = app.keymap.label(entry);
        let key = if label.is_empty() {
            t::help_palette_key(l)
        } else {
            label.as_str()
        };
        let key_w = 14.min(inner);
        let key_text = fit(key, key_w);
        let pad = " ".repeat(key_w.saturating_sub(key_text.width()) + 1);
        lines.push(Line::from(vec![
            Span::styled(key_text, theme.key()),
            Span::raw(pad),
            Span::raw(fit(entry.name(l), inner.saturating_sub(key_w + 1))),
        ]));
    }
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::help_title(l), lines, theme), area);
}

fn draw_palette(
    frame: &mut Frame<'_>,
    body: Rect,
    app: &App,
    input: &str,
    selected: usize,
    theme: Theme,
) {
    let l = app.locale;
    let matches = palette_matches(l, input);
    let shown = u16::try_from(matches.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let area = panel(
        body,
        body.width.saturating_sub(8).min(72),
        shown.min(body.height),
    );
    let inner = text_width(area);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("❯ ", theme.accent()),
            Span::raw(fit(&clean(input), inner.saturating_sub(3))),
            Span::styled("▏", theme.accent()),
        ]),
        Line::from(Span::styled("─".repeat(inner), theme.border())),
    ];
    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            t::palette_no_match(l),
            theme.dim(),
        )));
    }
    for (index, entry) in matches.iter().enumerate() {
        let key = app.keymap.label(entry);
        let key_w = key.width();
        let name = fit(entry.name(l), inner.saturating_sub(key_w + 4));
        let gap = inner.saturating_sub(name.width() + key_w + 2);
        let chosen = index == selected;
        let bar = if chosen {
            if theme.is_high_contrast() {
                "▶"
            } else {
                "▌"
            }
        } else {
            " "
        };
        let mut row = Line::from(vec![
            Span::styled(bar, theme.accent()),
            Span::raw(format!(" {name}")),
            Span::raw(" ".repeat(gap)),
            Span::styled(key, theme.key()),
        ]);
        if chosen {
            row = fill_line(row.style(theme.selected()), inner);
        }
        lines.push(row);
    }
    frame.render_widget(Clear, area);
    frame.render_widget(
        dialog(t::palette_title(l), lines, theme).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_search(frame: &mut Frame<'_>, body: Rect, input: &str, l: Locale, theme: Theme) {
    let area = panel(body, body.width.saturating_sub(8).min(64), 5);
    let inner = text_width(area);
    let lines = vec![
        Line::from(vec![
            Span::styled("/ ", theme.accent()),
            Span::raw(fit(&clean(input), inner.saturating_sub(3))),
            Span::styled("▏", theme.accent()),
        ]),
        hint_line(
            &[
                ("Enter", t::hint_filters_queue(l)),
                ("Esc", t::hint_cancels(l)),
            ],
            theme,
        ),
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::search_title(l), lines, theme), area);
}

fn draw_editor(frame: &mut Frame<'_>, body: Rect, editor: &Editor, l: Locale, theme: Theme) {
    let height = if editor.multiline { 12 } else { 7 };
    let area = panel(
        body,
        body.width.saturating_sub(6).min(76),
        height.min(body.height),
    );
    let inner = text_width(area);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if shows_severity(&editor.purpose) {
        lines.push(severity_line(editor.severity, l, theme));
        lines.push(Line::from(""));
    }
    let typed = format!("{}▏", clean_editor(&editor.text));
    for row in wrap(&typed, inner.max(1)) {
        lines.push(Line::from(Span::raw(row)));
    }
    if editor.text.is_empty() {
        lines.push(Line::from(Span::styled(t::editor_empty(l), theme.faint())));
    }
    if let Some(error) = &editor.error {
        lines.push(Line::from(Span::styled(
            fit(&clean(error), inner),
            theme.error(),
        )));
    }
    lines.push(Line::from(""));
    lines.push(if editor.multiline {
        hint_line(
            &[
                ("Enter", t::hint_saves(l)),
                ("Ctrl-J", t::hint_new_line(l)),
                ("Tab", t::hint_severity(l)),
                ("Esc", t::hint_cancels(l)),
            ],
            theme,
        )
    } else {
        hint_line(
            &[("Enter", t::hint_saves(l)), ("Esc", t::hint_cancels(l))],
            theme,
        )
    });

    frame.render_widget(Clear, area);
    frame.render_widget(dialog(&clean(&editor.title), lines, theme), area);
}

/// Editor text may be several lines, and every one of them is untrusted.
fn clean_editor(text: &str) -> String {
    text.split('\n').map(clean).collect::<Vec<_>>().join("\n")
}

fn shows_severity(purpose: &EditorPurpose) -> bool {
    matches!(
        purpose,
        EditorPurpose::NewComment { .. } | EditorPurpose::EditComment(_)
    )
}

/// `P0 P1 P2 P3 none`, with the current choice filled. The word is always there, not just the colour.
fn severity_line(current: Option<Priority>, l: Locale, theme: Theme) -> Line<'static> {
    let choices = [
        Some(Priority::P0),
        Some(Priority::P1),
        Some(Priority::P2),
        Some(Priority::P3),
        None,
    ];
    let mut spans = vec![Span::styled(t::severity_label(l), theme.dim())];
    for choice in choices {
        let label = choice.map_or(t::severity_none(l), Priority::label);
        let chosen = choice == current;
        let style = match (chosen, choice) {
            (true, _) if theme.is_high_contrast() => theme.selected(),
            (true, Some(p)) => theme.priority_chip(p),
            (true, None) => theme.chip(Chip::Quiet),
            (false, Some(p)) => theme.priority(p),
            (false, None) => theme.dim(),
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    Line::from(spans)
}

fn draw_confirm(frame: &mut Frame<'_>, body: Rect, confirm: &Confirm, l: Locale, theme: Theme) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let inner = text_width(area);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for row in &confirm.lines {
        for wrapped in wrap(&clean(row), inner.max(1)) {
            lines.push(Line::from(Span::raw(wrapped)));
        }
    }
    lines.push(Line::from(""));
    lines.push(hint_line(
        &[
            ("y / Enter", t::hint_confirms(l)),
            ("n / Esc", t::hint_cancels_nothing_sent(l)),
        ],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(&clean(&confirm.title), lines, theme), area);
}

/// A one-time explanation in a box. The text is wrapped to the box and never cut mid-word.
fn draw_notice(
    frame: &mut Frame<'_>,
    body: Rect,
    title: &str,
    text: &str,
    l: Locale,
    theme: Theme,
) {
    let width = body.width.saturating_sub(4).min(72);
    let inner = usize::from(width).saturating_sub(4);
    let mut lines: Vec<Line<'_>> = crate::tui::widgets::wrap(text, inner)
        .into_iter()
        .map(|row| Line::from(Span::raw(row)))
        .collect();
    lines.push(Line::from(Span::raw(String::new())));
    lines.push(Line::from(Span::styled(t::notice_continue(l), theme.dim())));
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(body.height);
    let area = panel(body, width, height);
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(title, lines, theme), area);
}

fn draw_explain(
    frame: &mut Frame<'_>,
    body: Rect,
    app: &App,
    unit: &crate::domain::ids::UnitId,
    theme: Theme,
) {
    let l = app.locale;
    let width = body.width.saturating_sub(4).min(78);
    let inner = usize::from(width).saturating_sub(4);
    let lines = match app.session.analysis().item(unit) {
        Some(item) => explain_lines(app, item, inner),
        None => vec![Line::from(Span::styled(t::explain_gone(l), theme.dim()))],
    };
    // As tall as the reasons need, so a short explanation does not float in an empty box.
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(body.height.saturating_sub(2));
    let area = panel(body, width, height);
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::explain_title(l), lines, theme), area);
}

/// The contract of one changed symbol. Source text is cleaned before it is drawn; a fact the
/// analysis could not establish is already the word "unknown" in the evidence.
fn draw_contract(
    frame: &mut Frame<'_>,
    body: Rect,
    app: &App,
    unit: &crate::domain::ids::UnitId,
    theme: Theme,
) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let l = app.locale;
    let inner = usize::from(area.width).saturating_sub(4);
    let lines = if let Some(delta) = app.session.contract(unit) {
        contract_lines(&delta, inner, l, theme)
    } else if let Some(file) = app.session.file_contract(unit) {
        file_contract_lines(&file, inner, l, theme)
    } else {
        vec![Line::from(Span::styled(t::contract_none(l), theme.dim()))]
    };
    let lines = clip_lines(lines, usize::from(area.height).saturating_sub(2), l, theme);
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::contract_title(l), lines, theme), area);
}

/// Keeps what fits the box and says how much was left out, so a short terminal never hides that
/// there was more to read.
fn clip_lines(
    mut lines: Vec<Line<'static>>,
    room: usize,
    l: Locale,
    theme: Theme,
) -> Vec<Line<'static>> {
    if room == 0 || lines.len() <= room {
        return lines;
    }
    let hidden = lines.len() - room + 1;
    lines.truncate(room.saturating_sub(1));
    lines.push(Line::from(Span::styled(
        t::clip_more_lines(l, hidden),
        theme.dim(),
    )));
    lines
}

fn contract_lines(
    delta: &ContractDelta,
    width: usize,
    l: Locale,
    theme: Theme,
) -> Vec<Line<'static>> {
    let none = t::contract_none_word(l);
    let mut lines = vec![
        labelled(
            t::contract_change(l),
            labels::change_kind(l, delta.change_kind),
            theme,
        ),
        labelled(
            t::contract_confidence(l),
            &format!("{:.2}", delta.confidence.value()),
            theme,
        ),
    ];
    if let Some(schema) = &delta.schema {
        lines.push(labelled(t::contract_data_type(l), schema, theme));
    }
    lines.push(labelled(
        t::contract_before(l),
        delta.before_signature.as_deref().unwrap_or(none),
        theme,
    ));
    lines.push(labelled(
        t::contract_after(l),
        delta.after_signature.as_deref().unwrap_or(none),
        theme,
    ));
    let parameters = Delta {
        before: delta
            .signature_before
            .as_ref()
            .map(|p| p.parameters.clone())
            .unwrap_or_default(),
        after: delta
            .signature_after
            .as_ref()
            .map(|p| p.parameters.clone())
            .unwrap_or_default(),
    };
    push_delta(
        &mut lines,
        t::contract_parameters(l),
        &parameters,
        width,
        theme,
    );
    let returns =
        |parts: &Option<crate::app::SignatureParts>| parts.as_ref().and_then(|p| p.returns.clone());
    match (
        returns(&delta.signature_before),
        returns(&delta.signature_after),
    ) {
        (Some(old), Some(new)) if old != new => {
            lines.push(labelled(
                t::contract_returns(l),
                &format!("{old} -> {new}"),
                theme,
            ));
        }
        (Some(old), None) => lines.push(labelled(
            t::contract_returns(l),
            &format!("{old} -> {none}"),
            theme,
        )),
        (None, Some(new)) => lines.push(labelled(
            t::contract_returns(l),
            &format!("{none} -> {new}"),
            theme,
        )),
        (Some(same), Some(_)) => lines.push(labelled(t::contract_returns(l), &same, theme)),
        (None, None) => {}
    }
    if delta.visibility_before != delta.visibility_after {
        lines.push(labelled(
            t::contract_visibility(l),
            &format!(
                "{} -> {}",
                delta.visibility_before.as_deref().unwrap_or(none),
                delta.visibility_after.as_deref().unwrap_or(none)
            ),
            theme,
        ));
    } else if let Some(same) = &delta.visibility_after {
        lines.push(labelled(t::contract_visibility(l), same, theme));
    }
    for (heading, items) in [
        (t::contract_decorators(l), &delta.decorators),
        (t::contract_endpoints(l), &delta.endpoints),
        (t::contract_raises(l), &delta.raises),
        (t::contract_effects(l), &delta.effects),
        (t::contract_members(l), &delta.members),
    ] {
        push_delta(&mut lines, heading, items, width, theme);
    }
    push_group(
        &mut lines,
        t::contract_reachable_from(l),
        &delta.reachable,
        t::contract_no_entrypoint(l),
        width,
        theme,
    );
    push_group(
        &mut lines,
        t::contract_direct_callers(l),
        &delta.affected,
        none,
        width,
        theme,
    );
    push_group(
        &mut lines,
        t::contract_evidence(l),
        &delta.evidence,
        none,
        width,
        theme,
    );
    lines.push(Line::from(Span::styled(
        fit(t::contract_any_key(l), width),
        theme.dim(),
    )));
    lines
}

/// A migration, a manifest or a settings file, as added, removed and changed entries.
fn file_contract_lines(
    file: &FileContract,
    width: usize,
    l: Locale,
    theme: Theme,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        labelled(t::contract_file(l), file.path.as_str(), theme),
        labelled(t::contract_kind(l), labels::file_kind(l, file.kind), theme),
        labelled(
            t::contract_read(l),
            if file.parsed {
                t::contract_yes(l)
            } else {
                t::contract_no_textual(l)
            },
            theme,
        ),
    ];
    let marked = |sign: &str, items: &[String]| -> Vec<String> {
        items.iter().map(|item| format!("{sign} {item}")).collect()
    };
    for (heading, sign, items) in [
        (t::contract_added(l), "+", &file.added),
        (t::contract_removed(l), "-", &file.removed),
        (t::contract_changed(l), "~", &file.changed),
    ] {
        if !items.is_empty() {
            push_group(&mut lines, heading, &marked(sign, items), "", width, theme);
        }
    }
    push_group(
        &mut lines,
        t::contract_notes(l),
        &file.notes,
        t::contract_none_word(l),
        width,
        theme,
    );
    lines.push(Line::from(Span::styled(
        fit(t::contract_any_key(l), width),
        theme.dim(),
    )));
    lines
}

/// A list that changed: `+` for what appeared, `-` for what went away and a plain row for what
/// stayed. Nothing is drawn when neither version has anything.
fn push_delta(
    lines: &mut Vec<Line<'static>>,
    heading: &str,
    delta: &Delta,
    width: usize,
    theme: Theme,
) {
    if delta.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(heading.to_owned(), theme.accent())));
    let rows = delta
        .added()
        .into_iter()
        .map(|item| ("+", item))
        .chain(delta.removed().into_iter().map(|item| ("-", item)))
        .chain(delta.kept().into_iter().map(|item| (" ", item)));
    for (sign, item) in rows {
        let style = match sign {
            "+" => theme.good(),
            "-" => theme.error(),
            _ => theme.text(),
        };
        for row in wrap(&format!("  {sign} {}", clean(item)), width.max(1)) {
            lines.push(Line::from(Span::styled(row, style)));
        }
    }
}

/// `label  value`, with the label dimmed and the value cleaned.
fn labelled(label: &str, value: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<18} "), theme.dim()),
        Span::raw(clean(value)),
    ])
}

/// A heading plus one cleaned row per item, wrapped to the panel width.
fn push_group(
    lines: &mut Vec<Line<'static>>,
    heading: &str,
    items: &[String],
    empty: &str,
    width: usize,
    theme: Theme,
) {
    lines.push(Line::from(Span::styled(heading.to_owned(), theme.accent())));
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            fit(&format!("  {empty}"), width),
            theme.dim(),
        )));
        return;
    }
    for item in items {
        let style = match item.trim_start().chars().next() {
            Some('+') if item.starts_with("+ ") => theme.good(),
            Some('-') if item.starts_with("- ") => theme.error(),
            Some('~') if item.starts_with("~ ") => theme.warn(),
            _ => theme.text(),
        };
        for row in wrap(&format!("  {}", clean(item)), width.max(1)) {
            lines.push(Line::from(Span::styled(row, style)));
        }
    }
}

fn draw_finish(frame: &mut Frame<'_>, body: Rect, stage: &FinishStage, l: Locale, theme: Theme) {
    match stage {
        FinishStage::Confirm { warnings } => draw_finish_confirm(frame, body, warnings, l, theme),
        FinishStage::Deliver { record, selected } => {
            draw_deliver(frame, body, record, *selected, l, theme);
        }
    }
}

fn draw_finish_confirm(
    frame: &mut Frame<'_>,
    body: Rect,
    warnings: &[String],
    l: Locale,
    theme: Theme,
) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let inner = usize::from(area.width).saturating_sub(4);
    let mut lines = vec![Line::from(Span::raw(fit(t::finish_intro(l), inner)))];
    if warnings.is_empty() {
        lines.push(Line::from(Span::styled(
            t::finish_no_warnings(l),
            theme.good(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            t::finish_warnings(l),
            theme.warn(),
        )));
        for warning in warnings {
            for row in wrap(&format!("· {}", clean(warning)), inner.max(1)) {
                lines.push(Line::from(Span::styled(row, theme.warn())));
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(hint_line(
        &[
            ("y / Enter", t::hint_finishes(l)),
            ("n / Esc", t::hint_goes_back(l)),
        ],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::finish_title(l), lines, theme), area);
}

fn draw_deliver(
    frame: &mut Frame<'_>,
    body: Rect,
    record: &FeedbackRecord,
    selected: usize,
    l: Locale,
    theme: Theme,
) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let inner = usize::from(area.width).saturating_sub(4);
    let mut lines = vec![
        Line::from(Span::styled(
            fit(
                &t::deliver_saved_as(l, &clean(record.feedback_id.as_str())),
                inner,
            ),
            theme.dim(),
        )),
        Line::from(""),
    ];
    for (index, option) in delivery_options(l).iter().enumerate() {
        lines.push(choice_row(option, index == selected, inner, theme));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(t::deliver_pasted(l), theme.dim())));
    for row in wrap(&clean(&record.prompt), inner.max(1)) {
        lines.push(Line::from(Span::raw(row)));
    }
    lines.push(Line::from(""));
    lines.push(hint_line(
        &[
            ("Enter", t::hint_chooses(l)),
            ("Esc", t::hint_closes_finished(l)),
        ],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(t::deliver_title(l), lines, theme), area);
}
