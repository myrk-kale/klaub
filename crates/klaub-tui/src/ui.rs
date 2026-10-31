//! Draws one frame of the reader.
//!
//! The layout, top to bottom: a title bar, a row of mode tabs, an optional banner when the
//! worktree drifted, the mode's own view, and a status line. Overlays (help, the palette, the
//! editor, a confirmation) are drawn last, centred over the mode view.
//!
//! Nothing drawn here comes from the repository, a comment or a document without passing through
//! [`clean`] first, so a hostile sequence in a symbol name cannot reach the terminal.

use klaub_core::finding::Priority;
use klaub_core::review::FeedbackRecord;
use klaub_core::snapshot::Drift;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::actions::ENTRIES;
use crate::app::{
    App, Confirm, DELIVERY_OPTIONS, Editor, EditorPurpose, FinishStage, Mode, Overlay, StatusKind,
};
use crate::theme::Theme;
use crate::update::palette_matches;
use crate::views::{draw_mode, explain_lines};
use crate::widgets::{clean, fit, wrap};

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
    draw_tabs(frame, tabs, app.mode, theme);
    if drift {
        draw_banner(frame, banner, theme);
    }
    draw_mode(frame, body, app);
    draw_status(frame, status, app, theme);

    if let Some(overlay) = app.overlay.clone() {
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
    let state = crate::labels::review_status(app.session.review().status);

    let width = usize::from(area.width);
    let right = format!(" {progress} reviewed · {state}");
    let left = format!(" {} · {scope} · {id}", clean(&repo));
    let line = Line::from(vec![
        Span::styled(fit(&left, width.saturating_sub(right.len())), theme.bar()),
        Span::styled(fit(&right, width), theme.bar().add_modifier(Modifier::BOLD)),
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

fn draw_tabs(frame: &mut Frame<'_>, area: Rect, current: Mode, theme: Theme) {
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
                theme.selected(),
            ))),
            area,
        );
        return;
    }

    let spans: Vec<Span<'static>> = Mode::ALL
        .iter()
        .zip(labels)
        .map(|(mode, label)| {
            let style = if *mode == current {
                theme.selected()
            } else {
                theme.dim()
            };
            Span::styled(label, style)
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_banner(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let text = " ! worktree changed since this snapshot — R takes a new one (asks first)";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            fit(text, usize::from(area.width)),
            theme.warn(),
        ))),
        area,
    );
}

// ---- status line ------------------------------------------------------------------------------------------------

fn draw_status(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let width = usize::from(area.width);
    let (text, style) = match &app.status {
        Some((text, kind)) => (clean(text), status_style(*kind, theme)),
        None if app.session.analysis().config.ui.show_hints => (
            hint_for(app.mode, app.filters.any()).to_owned(),
            theme.dim(),
        ),
        None => (String::new(), theme.dim()),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(fit(&text, width), style))),
        area,
    );
}

fn status_style(kind: StatusKind, theme: Theme) -> Style {
    match kind {
        StatusKind::Info => theme.text(),
        StatusKind::Warn => theme.warn(),
        StatusKind::Error => theme.error(),
    }
}

/// The keys worth knowing in the current mode. Shown only while no status message is pending.
fn hint_for(mode: Mode, filtered: bool) -> &'static str {
    let filter = if filtered { " · x clear filters" } else { "" };
    match mode {
        Mode::Queue => {
            if filter.is_empty() {
                " j/k move · Enter open · c comment · r reviewed · w why · / search · ? help · q q quit"
            } else {
                " j/k move · Enter open · c comment · r reviewed · w why · / search · x clear filters"
            }
        }
        Mode::Structure => " j/k move · Enter open · +/- depth · c comment · r reviewed · ? help",
        Mode::Code => {
            " j/k move · v old/new · [ ] change · c comment · V select · S symbol/file · h/l scroll"
        }
        Mode::Impact => " j/k move · Enter open the symbol · g back to the queue · ? help",
        Mode::Comments => " j/k move · Enter open the code · e edit · d delete a draft · ? help",
        Mode::Context => " j/k scroll · i set intent · a add criterion · ? help",
        Mode::Tests => " j/k move · X run (asks first) · U reload reports · ? help",
        Mode::Followup => {
            " j/k move · y resolved · p partial · u not resolved · X regression · ? help"
        }
        Mode::Coverage => " j/k move · Enter open a pending unit · ? help",
        Mode::Flow => " j/k scroll · p ask the agent for a diagram · L reload · ? help",
    }
}

// ---- overlays ---------------------------------------------------------------------------------------------------

fn draw_overlay(frame: &mut Frame<'_>, body: Rect, app: &App, overlay: &Overlay, theme: Theme) {
    match overlay {
        Overlay::Help => draw_help(frame, body, theme),
        Overlay::Palette { input, selected } => draw_palette(frame, body, input, *selected, theme),
        Overlay::Search { input } => draw_search(frame, body, input, theme),
        Overlay::Editor(editor) => draw_editor(frame, body, editor, theme),
        Overlay::Confirm(confirm) => draw_confirm(frame, body, confirm, theme),
        Overlay::Explain(unit) => draw_explain(frame, body, app, unit, theme),
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

fn boxed<'a>(title: &str, lines: Vec<Line<'a>>, theme: Theme) -> Paragraph<'a> {
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(theme.strong())
            .style(theme.text()),
    )
}

fn draw_help(frame: &mut Frame<'_>, body: Rect, theme: Theme) {
    let area = panel(
        body,
        body.width.saturating_sub(4),
        body.height.saturating_sub(2),
    );
    let inner = usize::from(area.width).saturating_sub(2);
    let mut lines = vec![Line::from(Span::styled(
        fit("Keys — press any key to close", inner),
        theme.dim(),
    ))];
    for entry in ENTRIES {
        let key = if entry.key.is_empty() {
            "palette"
        } else {
            entry.key
        };
        let row = format!("{key:<14} {}", entry.name);
        lines.push(Line::from(Span::raw(fit(&row, inner))));
    }
    frame.render_widget(Clear, area);
    frame.render_widget(boxed("Help", lines, theme), area);
}

fn draw_palette(frame: &mut Frame<'_>, body: Rect, input: &str, selected: usize, theme: Theme) {
    let matches = palette_matches(input);
    let shown = u16::try_from(matches.len())
        .unwrap_or(u16::MAX)
        .saturating_add(3);
    let area = panel(
        body,
        body.width.saturating_sub(8).min(72),
        shown.min(body.height),
    );
    let inner = usize::from(area.width).saturating_sub(2);
    let mut lines = vec![Line::from(vec![
        Span::styled(">", theme.strong()),
        Span::raw(format!(" {}", fit(&clean(input), inner.saturating_sub(2)))),
    ])];
    if matches.is_empty() {
        lines.push(Line::from(Span::styled("no match", theme.dim())));
    }
    for (index, entry) in matches.iter().enumerate() {
        let mark = if index == selected { "▶" } else { " " };
        let row = format!("{mark} {}   {}", entry.name, entry.key);
        let style = if index == selected {
            theme.selected()
        } else {
            theme.text()
        };
        lines.push(Line::from(Span::styled(fit(&row, inner), style)));
    }
    frame.render_widget(Clear, area);
    frame.render_widget(
        boxed("Actions", lines, theme).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_search(frame: &mut Frame<'_>, body: Rect, input: &str, theme: Theme) {
    let area = panel(body, body.width.saturating_sub(8).min(64), 5);
    let inner = usize::from(area.width).saturating_sub(4);
    let lines = vec![
        Line::from(vec![
            Span::styled("/", theme.strong()),
            Span::raw(format!(" {}", fit(&clean(input), inner))),
        ]),
        Line::from(Span::styled(
            "Enter filters the queue · Esc cancels",
            theme.dim(),
        )),
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(boxed("Search", lines, theme), area);
}

fn draw_editor(frame: &mut Frame<'_>, body: Rect, editor: &Editor, theme: Theme) {
    let height = if editor.multiline { 12 } else { 7 };
    let area = panel(
        body,
        body.width.saturating_sub(6).min(76),
        height.min(body.height),
    );
    let inner = usize::from(area.width).saturating_sub(4);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if shows_severity(&editor.purpose) {
        lines.push(severity_line(editor.severity, theme));
    }
    for row in wrap(&clean_editor(&editor.text), inner.max(1)) {
        lines.push(Line::from(Span::raw(row)));
    }
    if editor.text.is_empty() {
        lines.push(Line::from(Span::styled("(empty)", theme.dim())));
    }
    if let Some(error) = &editor.error {
        lines.push(Line::from(Span::styled(
            fit(&clean(error), inner),
            theme.error(),
        )));
    }
    let hints = if editor.multiline {
        "Enter saves · Ctrl-J new line · Tab severity · Esc cancels"
    } else {
        "Enter saves · Esc cancels"
    };
    lines.push(Line::from(Span::styled(fit(hints, inner), theme.dim())));

    frame.render_widget(Clear, area);
    frame.render_widget(boxed(&clean(&editor.title), lines, theme), area);
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

/// `P0 P1 P2 P3 none`, with the current choice reversed. The word is always there, not just the colour.
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
        let style = if chosen {
            theme.selected()
        } else {
            choice.map_or_else(|| theme.dim(), |p| theme.priority(p))
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
    let inner = usize::from(area.width).saturating_sub(4);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for row in &confirm.lines {
        for wrapped in wrap(&clean(row), inner.max(1)) {
            lines.push(Line::from(Span::raw(wrapped)));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        fit(
            "y / Enter confirms · n / Esc cancels · nothing is sent on its own",
            inner,
        ),
        theme.strong(),
    )));
    frame.render_widget(Clear, area);
    frame.render_widget(boxed(&clean(&confirm.title), lines, theme), area);
}

fn draw_explain(
    frame: &mut Frame<'_>,
    body: Rect,
    app: &App,
    unit: &klaub_core::ids::UnitId,
    theme: Theme,
) {
    let area = panel(
        body,
        body.width.saturating_sub(4).min(78),
        body.height.saturating_sub(2),
    );
    let inner = usize::from(area.width).saturating_sub(2);
    let item = app.session.analysis().item(unit);
    let lines = match item {
        Some(item) => explain_lines(app, item, inner),
        None => vec![Line::from(Span::styled(
            "This unit is no longer in the queue.",
            theme.dim(),
        ))],
    };
    frame.render_widget(Clear, area);
    frame.render_widget(boxed("Why this priority", lines, theme), area);
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
        lines.push(Line::from(Span::styled("No warnings.", theme.good())));
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
    lines.push(Line::from(Span::styled(
        "y / Enter finishes · n / Esc goes back",
        theme.strong(),
    )));
    frame.render_widget(Clear, area);
    frame.render_widget(boxed("Finish the review", lines, theme), area);
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
        let mark = if index == selected { "▶" } else { " " };
        let style = if index == selected {
            theme.selected()
        } else {
            theme.text()
        };
        lines.push(Line::from(Span::styled(format!("{mark} {option}"), style)));
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
    lines.push(Line::from(Span::styled(
        "Enter chooses · Esc closes (the review stays finished)",
        theme.dim(),
    )));
    frame.render_widget(Clear, area);
    frame.render_widget(boxed("Deliver the feedback", lines, theme), area);
}
