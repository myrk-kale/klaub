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
    App, Confirm, DELIVERY_OPTIONS, Editor, EditorPurpose, FinishStage, Mode, Overlay, StatusKind,
};
use crate::tui::keymap::Keymap;
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
        draw_too_small(frame, area, app.theme);
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
    let (title, tabs, banner, body, status) = (rows[0], rows[1], rows[2], rows[3], rows[4]);

    draw_title(frame, title, app, theme);
    app.tab_hits = draw_tabs(frame, tabs, app.mode, theme);
    if drift {
        draw_banner(frame, banner, theme);
    }
    draw_mode(frame, body, app);
    draw_status(frame, status, app, theme);

    if let Some(overlay) = app.overlay.clone() {
        if !theme.is_high_contrast() {
            frame
                .buffer_mut()
                .set_style(area, Style::default().add_modifier(Modifier::DIM));
        }
        draw_overlay(frame, body, app, &overlay, theme);
    }
}

fn draw_too_small(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let message = format!(
        "Terminal too small ({}×{}, need at least {MIN_WIDTH}×{MIN_HEIGHT}). Resize it, or press q twice to leave.",
        area.width, area.height,
    );
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
    let state = crate::tui::labels::review_status(app.session.review().status);

    let width = usize::from(area.width);
    let brand = " ◆ klaub ";
    let right_text = format!(" {progress} reviewed · {state} ");
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

/// Draws the row of mode tabs and returns where each one is, for the mouse.
fn draw_tabs(frame: &mut Frame<'_>, area: Rect, current: Mode, theme: Theme) -> Vec<(Rect, Mode)> {
    let width = usize::from(area.width);
    let labels: Vec<String> = Mode::ALL
        .iter()
        .enumerate()
        .map(|(i, mode)| format!(" {} {} ", i + 1, mode.label()))
        .collect();
    let full: usize = labels.iter().map(String::len).sum();

    // A narrow terminal shows only the current mode rather than a row of clipped names.
    if full > width {
        let label = format!(" {} ", current.label());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                fit(&label, width),
                theme.tab_active(),
            ))),
            area,
        );
        return Vec::new();
    }

    let mut hits = Vec::with_capacity(labels.len());
    let mut x = area.x;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (mode, label) in Mode::ALL.iter().zip(labels) {
        let cells = u16::try_from(label.len()).unwrap_or(u16::MAX);
        hits.push((Rect::new(x, area.y, cells, 1), *mode));
        x = x.saturating_add(cells);
        if *mode == current {
            spans.push(Span::styled(label, theme.tab_active()));
        } else {
            // The number is quieter than the name, so the row reads as names first.
            let (number, name) = label.split_at(3);
            spans.push(Span::styled(number.to_owned(), theme.faint()));
            spans.push(Span::styled(name.to_owned(), theme.tab_idle()));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    hits
}

fn draw_banner(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let text = " worktree changed since this snapshot, so impact may mix versions. R takes a new one (asks first)";
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
    let mode = format!(" {} ", app.mode.label().to_uppercase());
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
            for (index, (key, what)) in hints_for(app.mode, app.filters.any(), &app.keymap)
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
fn hints_for(mode: Mode, filtered: bool, keys: &Keymap) -> Vec<(String, String)> {
    let k = |id: &str| keys.key_for(id);
    let hint = |key: &dyn ToString, what: &str| (key.to_string(), what.to_owned());
    let moves = format!("{}/{}", k("down"), k("up"));
    match mode {
        Mode::Queue => vec![
            hint(&moves, "move"),
            hint(&"Enter".to_owned(), "open"),
            hint(&k("comment"), "comment"),
            hint(&k("toggle_reviewed"), "reviewed"),
            hint(&k("explain"), "why"),
            hint(&k("search"), "search"),
            if filtered {
                hint(&k("clear_filters"), "clear filters")
            } else {
                hint(&k("help"), "help")
            },
            hint(&"q q".to_owned(), "quit"),
        ],
        Mode::Structure => vec![
            hint(&moves, "move"),
            hint(&"Enter".to_owned(), "open"),
            hint(
                &format!("{}/{}", k("deeper_tree"), k("shallower_tree")),
                "depth",
            ),
            hint(&k("comment"), "comment"),
            hint(&k("toggle_reviewed"), "reviewed"),
            hint(&k("help"), "help"),
        ],
        Mode::Code => vec![
            hint(&moves, "move"),
            hint(&k("inspect_contract"), "contract"),
            hint(&k("open_file"), "file"),
            hint(&k("toggle_version"), "old/new"),
            hint(
                &format!("{} {}", k("previous_change"), k("next_change")),
                "change",
            ),
            hint(&k("comment"), "comment"),
            hint(&k("select_lines"), "select"),
            hint(
                &format!("{} {}", k("history_back"), k("history_forward")),
                "back/forward",
            ),
        ],
        Mode::Impact => vec![
            hint(&moves, "move"),
            hint(&"Enter".to_owned(), "open the symbol"),
            hint(&k("impact"), "back to the queue"),
            hint(&k("help"), "help"),
        ],
        Mode::Comments => vec![
            hint(&moves, "move"),
            hint(&"Enter".to_owned(), "open the code"),
            hint(&k("edit_comment"), "edit"),
            hint(&k("delete_comment"), "delete a draft"),
            hint(&k("help"), "help"),
        ],
        Mode::Context => vec![
            hint(&moves, "scroll"),
            hint(&k("edit_intent"), "set intent"),
            hint(&k("add_criterion"), "add criterion"),
            hint(&k("help"), "help"),
        ],
        Mode::Tests => vec![
            hint(&moves, "move"),
            hint(&k("run_tests"), "run (asks first)"),
            hint(&k("reload_reports"), "reload reports"),
            hint(&k("help"), "help"),
        ],
        Mode::Followup => vec![
            hint(&moves, "move"),
            hint(&k("decide_resolved"), "resolved"),
            hint(&k("decide_partial"), "partial"),
            hint(&k("decide_unresolved"), "not resolved"),
            hint(&k("decide_regression"), "regression"),
            hint(&k("help"), "help"),
        ],
        Mode::Coverage => vec![
            hint(&moves, "move"),
            hint(&"Enter".to_owned(), "open a pending unit"),
            hint(&k("help"), "help"),
        ],
        Mode::Flow => vec![
            hint(&moves, "scroll"),
            hint(&k("request_flow"), "ask the agent for a diagram"),
            hint(&k("reload_flow"), "reload"),
            hint(&k("help"), "help"),
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
        Overlay::Search { input } => draw_search(frame, body, input, theme),
        Overlay::Editor(editor) => draw_editor(frame, body, editor, theme),
        Overlay::Confirm(confirm) => draw_confirm(frame, body, confirm, theme),
        Overlay::Notice { title, text } => draw_notice(frame, body, title, text, theme),
        Overlay::Explain(unit) => draw_explain(frame, body, app, unit, theme),
        Overlay::Contract(unit) => draw_contract(frame, body, app, unit, theme),
        Overlay::Finish(stage) => draw_finish(frame, body, stage, theme),
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
    let area = panel(
        body,
        body.width.saturating_sub(4),
        body.height.saturating_sub(2),
    );
    let inner = text_width(area);
    let mut lines = vec![Line::from(Span::styled(
        fit("Keys — press any key to close", inner),
        theme.dim(),
    ))];
    for entry in ENTRIES {
        let label = app.keymap.label(entry);
        let key = if label.is_empty() {
            "palette"
        } else {
            label.as_str()
        };
        let key_w = 14.min(inner);
        let key_text = fit(key, key_w);
        let pad = " ".repeat(key_w.saturating_sub(key_text.width()) + 1);
        lines.push(Line::from(vec![
            Span::styled(key_text, theme.key()),
            Span::raw(pad),
            Span::raw(fit(entry.name, inner.saturating_sub(key_w + 1))),
        ]));
    }
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Help", lines, theme), area);
}

fn draw_palette(
    frame: &mut Frame<'_>,
    body: Rect,
    app: &App,
    input: &str,
    selected: usize,
    theme: Theme,
) {
    let matches = palette_matches(input);
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
        lines.push(Line::from(Span::styled("no match", theme.dim())));
    }
    for (index, entry) in matches.iter().enumerate() {
        let key = app.keymap.label(entry);
        let key_w = key.width();
        let name = fit(entry.name, inner.saturating_sub(key_w + 4));
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
        dialog("Actions", lines, theme).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_search(frame: &mut Frame<'_>, body: Rect, input: &str, theme: Theme) {
    let area = panel(body, body.width.saturating_sub(8).min(64), 5);
    let inner = text_width(area);
    let lines = vec![
        Line::from(vec![
            Span::styled("/ ", theme.accent()),
            Span::raw(fit(&clean(input), inner.saturating_sub(3))),
            Span::styled("▏", theme.accent()),
        ]),
        hint_line(
            &[("Enter", " filters the queue · "), ("Esc", " cancels")],
            theme,
        ),
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Search", lines, theme), area);
}

fn draw_editor(frame: &mut Frame<'_>, body: Rect, editor: &Editor, theme: Theme) {
    let height = if editor.multiline { 12 } else { 7 };
    let area = panel(
        body,
        body.width.saturating_sub(6).min(76),
        height.min(body.height),
    );
    let inner = text_width(area);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if shows_severity(&editor.purpose) {
        lines.push(severity_line(editor.severity, theme));
        lines.push(Line::from(""));
    }
    let typed = format!("{}▏", clean_editor(&editor.text));
    for row in wrap(&typed, inner.max(1)) {
        lines.push(Line::from(Span::raw(row)));
    }
    if editor.text.is_empty() {
        lines.push(Line::from(Span::styled("(empty)", theme.faint())));
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
                ("Enter", " saves · "),
                ("Ctrl-J", " new line · "),
                ("Tab", " severity · "),
                ("Esc", " cancels"),
            ],
            theme,
        )
    } else {
        hint_line(&[("Enter", " saves · "), ("Esc", " cancels")], theme)
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
fn severity_line(current: Option<Priority>, theme: Theme) -> Line<'static> {
    let choices = [
        Some(Priority::P0),
        Some(Priority::P1),
        Some(Priority::P2),
        Some(Priority::P3),
        None,
    ];
    let mut spans = vec![Span::styled("severity ", theme.dim())];
    for choice in choices {
        let label = choice.map_or("none", Priority::label);
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

fn draw_confirm(frame: &mut Frame<'_>, body: Rect, confirm: &Confirm, theme: Theme) {
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
            ("y / Enter", " confirms · "),
            ("n / Esc", " cancels · nothing is sent on its own"),
        ],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog(&clean(&confirm.title), lines, theme), area);
}

/// A one-time explanation in a box. The text is wrapped to the box and never cut mid-word.
fn draw_notice(frame: &mut Frame<'_>, body: Rect, title: &str, text: &str, theme: Theme) {
    let width = body.width.saturating_sub(4).min(72);
    let inner = usize::from(width).saturating_sub(4);
    let mut lines: Vec<Line<'_>> = crate::tui::widgets::wrap(text, inner)
        .into_iter()
        .map(|row| Line::from(Span::raw(row)))
        .collect();
    lines.push(Line::from(Span::raw(String::new())));
    lines.push(Line::from(Span::styled(
        "Press any key to continue.",
        theme.dim(),
    )));
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
    let width = body.width.saturating_sub(4).min(78);
    let inner = usize::from(width).saturating_sub(4);
    let lines = match app.session.analysis().item(unit) {
        Some(item) => explain_lines(app, item, inner),
        None => vec![Line::from(Span::styled(
            "This unit is no longer in the queue.",
            theme.dim(),
        ))],
    };
    // As tall as the reasons need, so a short explanation does not float in an empty box.
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(body.height.saturating_sub(2));
    let area = panel(body, width, height);
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Priority", lines, theme), area);
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
    let inner = usize::from(area.width).saturating_sub(4);
    let lines = if let Some(delta) = app.session.contract(unit) {
        contract_lines(&delta, inner, theme)
    } else if let Some(file) = app.session.file_contract(unit) {
        file_contract_lines(&file, inner, theme)
    } else {
        vec![Line::from(Span::styled(
            "This unit has no contract to inspect: it is not a symbol, a migration, a dependency manifest or a settings file.",
            theme.dim(),
        ))]
    };
    let lines = clip_lines(lines, usize::from(area.height).saturating_sub(2), theme);
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Contract", lines, theme), area);
}

/// Keeps what fits the box and says how much was left out, so a short terminal never hides that
/// there was more to read.
fn clip_lines(mut lines: Vec<Line<'static>>, room: usize, theme: Theme) -> Vec<Line<'static>> {
    if room == 0 || lines.len() <= room {
        return lines;
    }
    let hidden = lines.len() - room + 1;
    lines.truncate(room.saturating_sub(1));
    lines.push(Line::from(Span::styled(
        format!("… {hidden} more lines; make the terminal taller"),
        theme.dim(),
    )));
    lines
}

fn contract_lines(delta: &ContractDelta, width: usize, theme: Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        labelled("change", delta.change_kind.label(), theme),
        labelled(
            "confidence",
            &format!("{:.2}", delta.confidence.value()),
            theme,
        ),
    ];
    if let Some(schema) = &delta.schema {
        lines.push(labelled("data type", schema, theme));
    }
    lines.push(labelled(
        "before",
        delta.before_signature.as_deref().unwrap_or("none"),
        theme,
    ));
    lines.push(labelled(
        "after",
        delta.after_signature.as_deref().unwrap_or("none"),
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
    push_delta(&mut lines, "parameters", &parameters, width, theme);
    let returns =
        |parts: &Option<crate::app::SignatureParts>| parts.as_ref().and_then(|p| p.returns.clone());
    match (
        returns(&delta.signature_before),
        returns(&delta.signature_after),
    ) {
        (Some(old), Some(new)) if old != new => {
            lines.push(labelled("returns", &format!("{old} -> {new}"), theme));
        }
        (Some(old), None) => lines.push(labelled("returns", &format!("{old} -> none"), theme)),
        (None, Some(new)) => lines.push(labelled("returns", &format!("none -> {new}"), theme)),
        (Some(same), Some(_)) => lines.push(labelled("returns", &same, theme)),
        (None, None) => {}
    }
    if delta.visibility_before != delta.visibility_after {
        lines.push(labelled(
            "visibility",
            &format!(
                "{} -> {}",
                delta.visibility_before.as_deref().unwrap_or("none"),
                delta.visibility_after.as_deref().unwrap_or("none")
            ),
            theme,
        ));
    } else if let Some(same) = &delta.visibility_after {
        lines.push(labelled("visibility", same, theme));
    }
    push_delta(&mut lines, "decorators", &delta.decorators, width, theme);
    push_delta(&mut lines, "endpoints", &delta.endpoints, width, theme);
    push_delta(&mut lines, "raises", &delta.raises, width, theme);
    push_delta(&mut lines, "effects", &delta.effects, width, theme);
    push_delta(&mut lines, "members", &delta.members, width, theme);
    push_group(
        &mut lines,
        "reachable from",
        &delta.reachable,
        "no entrypoint found (absence of a path is not proof of no impact)",
        width,
        theme,
    );
    push_group(
        &mut lines,
        "direct callers",
        &delta.affected,
        "none",
        width,
        theme,
    );
    push_group(
        &mut lines,
        "evidence",
        &delta.evidence,
        "none",
        width,
        theme,
    );
    lines.push(Line::from(Span::styled(
        fit("any key closes", width),
        theme.dim(),
    )));
    lines
}

/// A migration, a manifest or a settings file, as added, removed and changed entries.
fn file_contract_lines(file: &FileContract, width: usize, theme: Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        labelled("file", file.path.as_str(), theme),
        labelled("kind", file.kind.label(), theme),
        labelled(
            "read",
            if file.parsed {
                "yes"
            } else {
                "no, use the textual diff"
            },
            theme,
        ),
    ];
    let marked = |sign: &str, items: &[String]| -> Vec<String> {
        items.iter().map(|item| format!("{sign} {item}")).collect()
    };
    for (heading, sign, items) in [
        ("added", "+", &file.added),
        ("removed", "-", &file.removed),
        ("changed", "~", &file.changed),
    ] {
        if !items.is_empty() {
            push_group(&mut lines, heading, &marked(sign, items), "", width, theme);
        }
    }
    push_group(&mut lines, "notes", &file.notes, "none", width, theme);
    lines.push(Line::from(Span::styled(
        fit("any key closes", width),
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

fn draw_finish(frame: &mut Frame<'_>, body: Rect, stage: &FinishStage, theme: Theme) {
    match stage {
        FinishStage::Confirm { warnings } => draw_finish_confirm(frame, body, warnings, theme),
        FinishStage::Deliver { record, selected } => {
            draw_deliver(frame, body, record, *selected, theme)
        }
    }
}

fn draw_finish_confirm(frame: &mut Frame<'_>, body: Rect, warnings: &[String], theme: Theme) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let inner = usize::from(area.width).saturating_sub(4);
    let mut lines = vec![Line::from(Span::raw(fit(
        "Finishing freezes the comments into a feedback file. It sends nothing.",
        inner,
    )))];
    if warnings.is_empty() {
        lines.push(Line::from(Span::styled("✓ No warnings.", theme.good())));
    } else {
        lines.push(Line::from(Span::styled(
            "Warnings (they do not block):",
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
        &[("y / Enter", " finishes · "), ("n / Esc", " goes back")],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Finish the review", lines, theme), area);
}

fn draw_deliver(
    frame: &mut Frame<'_>,
    body: Rect,
    record: &FeedbackRecord,
    selected: usize,
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
                &format!("Saved as {}", clean(record.feedback_id.as_str())),
                inner,
            ),
            theme.dim(),
        )),
        Line::from(""),
    ];
    for (index, option) in DELIVERY_OPTIONS.iter().enumerate() {
        lines.push(choice_row(option, index == selected, inner, theme));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "The instruction that would be pasted:",
        theme.dim(),
    )));
    for row in wrap(&clean(&record.prompt), inner.max(1)) {
        lines.push(Line::from(Span::raw(row)));
    }
    lines.push(Line::from(""));
    lines.push(hint_line(
        &[
            ("Enter", " chooses · "),
            ("Esc", " closes (the review stays finished)"),
        ],
        theme,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog("Deliver the feedback", lines, theme), area);
}
