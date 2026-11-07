//! The main area for each mode. Everything that comes from the repository, a comment or a document
//! goes through [`clean`] before it reaches the screen.

use std::collections::HashSet;

use crate::app::CodeLine;
use crate::domain::config::Density;
use crate::domain::finding::QueueItem;
use crate::domain::flow::FlowState;
use crate::domain::i18n::tui as t;
use crate::domain::review::{CommentStatus, CriterionStatus, FollowupStatus, Version};
use crate::domain::semantic::LineMark;
use crate::domain::testing::EvidenceStatus;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::tui::app::{App, ImpactRowKind, Mode, TreeKind};
use crate::tui::labels;
use crate::tui::theme::{Chip, Theme, mark_symbol};
use crate::tui::widgets::{
    chip_span, clean, draw_cards, draw_list, empty_state, fill_line, fit, fit_tail, gauge, inset,
    pad, panel, panel_with, percent, scrollbar, vline, wrap,
};

/// Draws the main area for the current mode.
pub(crate) fn draw_mode(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    match app.mode {
        Mode::Queue | Mode::Structure | Mode::Files | Mode::Code => workspace(frame, area, app),
        Mode::Impact => impact(frame, area, app),
        Mode::Comments => comments(frame, area, app),
        Mode::Context => context(frame, area, app),
        Mode::Tests => tests(frame, area, app),
        Mode::Followup => followup(frame, area, app),
        Mode::Coverage => coverage(frame, area, app),
        Mode::Flow => flow(frame, area, app),
    }
}

/// A one-line rule across `area`, to separate a detail pane from the list above it.
fn rule(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let line = Line::from(Span::styled(
        "─".repeat(usize::from(area.width)),
        theme.border(),
    ));
    frame.render_widget(Paragraph::new(line), Rect { height: 1, ..area });
}

/// The status of a comment as a coloured word.
fn comment_status_style(status: CommentStatus, theme: Theme) -> Style {
    match status {
        CommentStatus::Draft => theme.dim(),
        CommentStatus::Prepared => theme.accent(),
        CommentStatus::Submitted | CommentStatus::Addressed => theme.good(),
        CommentStatus::Unresolved | CommentStatus::Stale => theme.warn(),
    }
}

/// Lines per row for the user's density: `(compact, comfortable)`.
fn stride(app: &App, compact: usize, comfortable: usize) -> usize {
    match app.session.analysis().config.ui.density {
        Density::Compact => compact,
        Density::Comfortable => comfortable,
    }
}

// ---- queue ------------------------------------------------------------------------------------------------

/// The list pane for the list the reader chose: the queue, the tree or the flat list of files.
fn list_pane(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    match app.list {
        Mode::Structure => structure(frame, area, app),
        Mode::Files => files(frame, area, app),
        _ => queue(frame, area, app),
    }
}

/// The review workspace: a list on the left and the code of the selected unit on the right. The
/// queue, the tree and the file list are three ways to fill the left side, and moving in any of
/// them shows the code at once. A terminal too narrow for both shows the pane that has the focus.
fn workspace(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    if matches!(app.mode, Mode::Queue | Mode::Structure | Mode::Files) {
        app.list = app.mode;
    }
    sync_preview(app);
    let focus_code = app.mode == Mode::Code;
    if area.width < 100 {
        if focus_code {
            code(frame, area, app);
        } else {
            list_pane(frame, area, app);
        }
        return;
    }
    let left_w = (area.width * 30 / 100).clamp(36, 56);
    let left = Rect {
        width: left_w,
        ..area
    };
    let divider = Rect {
        x: area.x + left_w + 1,
        width: 1,
        ..area
    };
    let right = Rect {
        x: area.x + left_w + 3,
        width: area.width - left_w - 3,
        ..area
    };
    list_pane(frame, left, app);
    vline(frame, divider, theme);
    code(frame, right, app);
}

/// Shows the code of the unit under the list cursor while a list has the focus, so the right side
/// follows the left without a key press.
fn sync_preview(app: &mut App) {
    if app.mode == Mode::Code {
        return;
    }
    let Some(unit) = app.unit.clone().or_else(|| app.queue_selection()) else {
        return;
    };
    if app.unit.is_none() {
        app.adopt(unit.clone());
    }
    let shown = app.code.view.as_ref().and_then(|v| v.unit.as_ref());
    if shown != Some(&unit) {
        app.preview_unit(&unit);
    }
}

/// The header of the left pane: the three lists, the one in use marked, and a line of counts.
/// Returns the area under it.
fn list_header(frame: &mut Frame<'_>, area: Rect, app: &App, counts: &str) -> Rect {
    let theme = app.theme;
    let l = app.locale;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (mode, id) in [
        (Mode::Queue, "queue"),
        (Mode::Structure, "structure"),
        (Mode::Files, "files"),
    ] {
        let name = format!("{} {}", app.keymap.key_for(id), mode.label(l));
        if mode == app.list {
            spans.push(Span::styled(
                name,
                theme.accent().add_modifier(Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::styled(name, theme.faint()));
        }
        spans.push(Span::raw("   "));
    }
    let head = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(Line::from(spans)), head);
    let sub = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            fit(counts, usize::from(area.width)),
            theme.faint(),
        ))),
        sub,
    );
    Rect {
        y: area.y.saturating_add(3),
        height: area.height.saturating_sub(3),
        ..area
    }
}

fn queue(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let total = app.session.analysis().queue.len();
    let visible = app.visible_queue().len();
    let summary = if app.filters.any() {
        t::queue_filter_summary(l, &app.filters.summary(l))
    } else {
        String::new()
    };
    let inner = list_header(
        frame,
        area,
        app,
        &t::list_count(l, visible, total, &summary),
    );
    if visible == 0 {
        let text = if total == 0 {
            t::queue_nothing_changed(l)
        } else {
            t::queue_no_match(l)
        };
        empty_state(frame, inner, text, theme);
        return;
    }
    let width = usize::from(inner.width).saturating_sub(2);
    let rows: Vec<Vec<Line<'static>>> = app
        .visible_queue()
        .iter()
        .map(|item| queue_row(app, item, width))
        .collect();
    let lines = stride(app, 2, 3);
    draw_cards(frame, inner, &mut app.queue_view, rows, lines, theme);
}

/// The changed files as one flat list: the file name first, the folder under it.
fn files(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let rows = app.files_rows();
    let inner = list_header(frame, area, app, &t::files_count(l, rows.len()));
    if rows.is_empty() {
        empty_state(frame, inner, t::queue_nothing_changed(l), theme);
        return;
    }
    let width = usize::from(inner.width).saturating_sub(2);
    let cards: Vec<Vec<Line<'static>>> = rows
        .iter()
        .map(|row| {
            let (done, all) = row.progress;
            let progress = if done == all {
                "✓".to_owned()
            } else {
                format!("{done}/{all}")
            };
            let progress_style = if done == all {
                theme.good()
            } else {
                theme.faint()
            };
            let (dir, name) = row
                .path
                .rsplit_once('/')
                .map_or((String::new(), row.path.as_str()), |(d, n)| {
                    (format!("{d}/"), n)
                });
            let lead = 6;
            let name_w = width.saturating_sub(lead + progress.width() + 2);
            let top = Line::from(vec![
                Span::raw("  "),
                Span::styled(row.priority.label(), theme.priority(row.priority)),
                Span::raw("  "),
                Span::styled(pad(&clean(name), name_w), theme.strong()),
                Span::styled(format!("  {progress}"), progress_style),
            ]);
            let under = Line::from(vec![
                Span::raw(" ".repeat(lead)),
                Span::styled(
                    fit_tail(&clean(&dir), width.saturating_sub(lead)),
                    theme.faint(),
                ),
            ]);
            vec![top, under]
        })
        .collect();
    let lines = stride(app, 2, 3);
    draw_cards(frame, inner, &mut app.files_view, cards, lines, theme);
}

/// One queue entry as a card: the name on top, what kind of change it is and why it matters under
/// it. The file is not repeated here: the code pane already names it.
fn queue_row(app: &App, item: &QueueItem, width: usize) -> Vec<Line<'static>> {
    let theme = app.theme;
    let l = app.locale;
    let reviewed = app.session.review().is_reviewed(&item.unit.id);
    let comments = app.session.comments_on(&item.unit.id).len();
    // The check and the priority, then two spaces: the second line starts under the name.
    let lead = 6;
    let note = if comments > 0 {
        format!("  ✎{comments}")
    } else {
        String::new()
    };
    let label_w = width.saturating_sub(lead + note.width());
    let label_style = if reviewed {
        theme.dim()
    } else {
        theme.strong()
    };
    let top = Line::from(vec![
        Span::styled(if reviewed { "✓ " } else { "  " }, theme.good()),
        Span::styled(item.priority.label(), theme.priority(item.priority)),
        Span::raw("  "),
        Span::styled(pad(&clean(&item.unit.label), label_w), label_style),
        Span::styled(note, theme.warn()),
    ]);

    let mut facts = vec![labels::change_kind(l, item.change)];
    facts.extend(item.tags.iter().map(|tag| labels::queue_tag(l, *tag)));
    let under = Line::from(vec![
        Span::raw(" ".repeat(lead)),
        Span::styled(
            fit(&facts.join(" · "), width.saturating_sub(lead)),
            theme.dim(),
        ),
    ]);
    vec![top, under]
}

/// The reasons behind a priority, one line each.
pub(crate) fn explain_lines(app: &App, item: &QueueItem, width: usize) -> Vec<Line<'static>> {
    let theme = app.theme;
    let l = app.locale;
    let score = t::explain_score(l, item.score.total(), &clean(&item.unit.label));
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, chunk) in wrap(&score, width.saturating_sub(4))
        .into_iter()
        .enumerate()
    {
        let lead = if i == 0 {
            Span::styled(
                format!("{}  ", item.priority.label()),
                theme.priority(item.priority),
            )
        } else {
            Span::raw("    ")
        };
        lines.push(Line::from(vec![lead, Span::styled(chunk, theme.strong())]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(t::explain_why(l), theme.accent())));
    for part in item.score.parts.iter().filter(|p| p.points != 0) {
        let text = format!(
            "{:+} {} — {}",
            part.points,
            labels::score_component(l, part.component),
            clean(&part.reason)
        );
        for (i, chunk) in wrap(&text, width.saturating_sub(6)).into_iter().enumerate() {
            let points = part.points;
            let style = match points {
                1.. => theme.warn(),
                _ => theme.good(),
            };
            let indent = if i == 0 { "  " } else { "      " };
            if i == 0 {
                // The signed number is the part to scan; the reason is read after it.
                let (number, reason) = chunk.split_once(' ').unwrap_or((&chunk, ""));
                lines.push(Line::from(vec![
                    Span::raw(indent),
                    Span::styled(number.to_owned(), style.add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" {reason}")),
                ]));
            } else {
                lines.push(Line::from(format!("{indent}{chunk}")));
            }
        }
    }
    if item.score.parts.iter().all(|p| p.points == 0) {
        lines.push(Line::from(Span::styled(
            t::explain_no_factor(l),
            theme.dim(),
        )));
    }
    if !item.findings.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            t::explain_findings(l),
            theme.accent(),
        )));
        for (number, finding) in item.findings.iter().enumerate() {
            let temporal = labels::temporal(l, finding.temporal);
            let text = format!(
                "{} [{temporal}] {} ({})",
                if number < 9 {
                    format!("{}", number + 1)
                } else {
                    "-".to_owned()
                },
                clean(&finding.title),
                clean(&finding.rule_id)
            );
            for chunk in wrap(&text, width.saturating_sub(2)) {
                lines.push(Line::from(format!("  {chunk}")));
            }
            for evidence in finding.evidence.iter().take(2) {
                let place = evidence
                    .span
                    .map_or_else(String::new, |s| format!(":{}", s.start_line));
                lines.push(Line::from(Span::styled(
                    fit(
                        &format!(
                            "    ↳ {}{place} {}",
                            clean(evidence.file.as_str()),
                            clean(&evidence.detail)
                        ),
                        width,
                    ),
                    theme.dim(),
                )));
            }
        }
    }
    if !item.findings.is_empty() {
        for chunk in wrap(t::explain_false_positive(l), width) {
            lines.push(Line::from(Span::styled(chunk, theme.dim())));
        }
    }
    if !app.session.muted_rules().is_empty() {
        lines.push(Line::from(Span::styled(
            fit(
                &t::explain_silenced(l, &clean(&app.session.muted_rules().join(", "))),
                width,
            ),
            theme.dim(),
        )));
    }
    if let Some(scope) = item.scope {
        lines.push(Line::from(Span::styled(
            t::explain_reach(l, labels::reach(l, scope), item.entrypoints),
            theme.dim(),
        )));
    }
    lines
}

// ---- structure -------------------------------------------------------------------------------------------

fn structure(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let body = list_header(frame, area, app, t::structure_hint(l));
    let tree = app.tree_rows();
    if tree.is_empty() {
        empty_state(frame, body, t::structure_empty(l), theme);
        return;
    }
    let width = usize::from(body.width).saturating_sub(2);
    let rows = tree_cards(app, &tree, width);
    let lines = stride(app, 1, 2);
    draw_cards(frame, body, &mut app.structure_view, rows, lines, theme);
}

/// The tree as cards. Each row draws its branch (`├─`, `╰─`) and the guides of the branches above
/// it that go on; a second line, when the density gives one, carries those guides across the gap so
/// the tree stays connected.
fn tree_cards(
    app: &App,
    tree: &[crate::tui::app::TreeRow],
    width: usize,
) -> Vec<Vec<Line<'static>>> {
    let theme = app.theme;
    let count = tree.len();
    // later[i]: a row at the same depth follows, before anything shallower does.
    let mut later = vec![false; count];
    let mut seen: Vec<bool> = Vec::new();
    for i in (0..count).rev() {
        let depth = tree[i].depth;
        if seen.len() <= depth {
            seen.resize(depth + 1, false);
        }
        later[i] = seen[depth];
        seen[depth] = true;
        for deeper in seen.iter_mut().skip(depth + 1) {
            *deeper = false;
        }
    }

    let cell = |on: bool| if on { "│  " } else { "   " };
    let mut ancestors: Vec<bool> = Vec::new();
    let mut parents: Vec<String> = Vec::new();
    let mut cards = Vec::with_capacity(count);
    for (i, row) in tree.iter().enumerate() {
        let depth = row.depth;
        ancestors.truncate(depth);
        parents.truncate(depth);
        let has_children = tree.get(i + 1).is_some_and(|next| next.depth > depth);

        let mut guides = String::new();
        for level in 1..depth {
            guides.push_str(cell(ancestors.get(level).copied().unwrap_or(false)));
        }
        if depth >= 1 {
            guides.push_str(if later[i] { "├─ " } else { "╰─ " });
        }

        // Only the urgent rows say their priority; a dot is enough for the rest.
        let (mark, mark_style) = match row.priority {
            Some(p) if p.is_high() => (p.label().to_owned(), theme.priority(p)),
            _ => ("· ".to_owned(), theme.faint()),
        };

        let (icon, icon_style) = match &row.kind {
            TreeKind::Module(_) => ("▾ ", theme.accent()),
            TreeKind::File(path) if app.collapsed.contains(path) => ("▸ ", theme.dim()),
            TreeKind::File(_) => ("▾ ", theme.dim()),
            TreeKind::Unit(_) if row.progress.0 >= 1 => ("✓ ", theme.good()),
            TreeKind::Unit(_) => ("○ ", theme.faint()),
        };

        // A file shows its name first and the folder after it, without repeating the module above.
        let label = clean(&row.label);
        let (name, dir) = match &row.kind {
            TreeKind::File(_) => {
                let relative = parents
                    .last()
                    .and_then(|module| label.strip_prefix(&format!("{module}/")))
                    .unwrap_or(&label);
                match relative.rsplit_once('/') {
                    Some((dir, name)) => (name.to_owned(), format!("{dir}/")),
                    None => (relative.to_owned(), String::new()),
                }
            }
            _ => (label.clone(), String::new()),
        };
        let name_style = match &row.kind {
            TreeKind::Module(_) => theme.accent(),
            TreeKind::File(_) => theme.strong(),
            TreeKind::Unit(_) => theme.text(),
        };

        let (done, all) = row.progress;
        let (progress, progress_style) = match &row.kind {
            TreeKind::Unit(_) => (String::new(), theme.dim()),
            _ if done == all && all > 0 => ("✓".to_owned(), theme.good()),
            _ => (format!("{done}/{all}"), theme.faint()),
        };

        let lead = mark.width().max(2) + 1;
        let used = lead + guides.width() + icon.width() + progress.width() + 2;
        let room = width.saturating_sub(used);
        let name = fit(&name, room);
        let dir = fit_tail(&dir, room.saturating_sub(name.width() + 2));
        let gap =
            room.saturating_sub(name.width() + dir.width() + usize::from(!dir.is_empty()) * 2);

        let mut spans = vec![
            Span::styled(pad(&mark, 2), mark_style),
            Span::raw(" "),
            Span::styled(guides, theme.border()),
            Span::styled(icon, icon_style),
            Span::styled(name, name_style),
        ];
        if !dir.is_empty() {
            spans.push(Span::styled(format!("  {dir}"), theme.faint()));
        }
        spans.push(Span::raw(" ".repeat(gap + 2)));
        spans.push(Span::styled(progress, progress_style));

        // The line under the row: the same guides, so the branches run through the gap.
        let mut under = String::new();
        for level in 1..depth {
            under.push_str(cell(ancestors.get(level).copied().unwrap_or(false)));
        }
        if depth >= 1 {
            under.push_str(cell(later[i]));
        }
        under.push_str(cell(has_children));
        let under = Line::from(vec![
            Span::raw(" ".repeat(3)),
            Span::styled(under.trim_end().to_owned(), theme.border()),
        ]);

        cards.push(vec![Line::from(spans), under]);
        ancestors.push(later[i]);
        if matches!(row.kind, TreeKind::Module(_)) {
            parents.push(label);
        } else {
            parents.push(parents.last().cloned().unwrap_or_default());
        }
    }
    cards
}

// ---- code ----------------------------------------------------------------------------------------------------

fn code(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let Some(view) = app.code.view.clone() else {
        let body = panel(frame, area, t::mode_code(l), "", theme);
        empty_state(frame, body, t::code_empty(l), theme);
        return;
    };

    let (side, side_chip) = match view.version {
        Version::After => (t::code_new(l), Chip::Good),
        Version::Before => (t::code_old(l), Chip::Warn),
    };
    let place = view
        .symbol_lines
        .map_or_else(String::new, |(a, b)| t::code_lines(l, a, b));
    let title = format!(
        "{} · {}{place}",
        clean(view.path.as_str()),
        app.current_item()
            .map_or_else(|| t::code_symbol(l).to_owned(), |i| clean(&i.unit.label)),
    );
    let badge = chip_span(side, theme.chip(side_chip));
    let inner = panel_with(frame, area, &title, "", vec![badge], theme);

    // A notice (the file is binary, the version is missing...) takes the first row.
    let notice_h = u16::from(view.notice.is_some());
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(notice_h), Constraint::Min(1)])
        .split(inner);
    if let Some(notice) = &view.notice {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ▲ ", theme.warn()),
                Span::styled(
                    fit(
                        &clean(notice),
                        usize::from(parts[0].width).saturating_sub(3),
                    ),
                    theme.warn(),
                ),
            ])),
            parts[0],
        );
    }
    let (body, side) = split_inspector(app, parts[1]);
    if let Some(side) = side {
        draw_inspector(frame, side, app, &view);
    }

    if view.lines.is_empty() {
        return;
    }

    // Comments that touch the cursor line are shown under the code.
    let (commented, at_cursor) = comment_lines(app, &view);
    let footer_h: u16 = if at_cursor.is_empty() { 0 } else { 2 };
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(footer_h)])
        .split(body);
    let (text_area, footer_area) = (split[0], split[1]);
    app.code.height = usize::from(text_area.height);
    app.code.area = text_area;
    crate::tui::actions::ensure_code_visible(app);

    let number_w = view
        .lines
        .last()
        .map_or(3, |l| l.number.to_string().len())
        .max(3);
    let gutter_w = number_w + 4;
    let text_w = usize::from(text_area.width).saturating_sub(gutter_w);
    let selection = app
        .code
        .selection
        .map(|s| (s.min(app.code.cursor), s.max(app.code.cursor)));
    let lines: Vec<Line<'static>> = view
        .lines
        .iter()
        .enumerate()
        .skip(app.code.offset)
        .take(usize::from(text_area.height))
        .map(|(i, line)| {
            let is_cursor = i == app.code.cursor && app.mode == Mode::Code;
            let mark = mark_symbol(line.mark);
            let dot = if commented.contains(&line.number) {
                '●'
            } else {
                ' '
            };
            let number_style = if is_cursor {
                theme.accent()
            } else {
                theme.faint()
            };
            let outside = if line.in_symbol {
                Modifier::empty()
            } else {
                Modifier::DIM
            };
            let mut spans = vec![
                Span::styled(mark.to_string(), theme.mark(line.mark)),
                Span::styled(
                    format!("{:>number_w$} ", line.number),
                    number_style.add_modifier(outside),
                ),
                Span::styled(dot.to_string(), theme.warn()),
                Span::styled("│", theme.border()),
            ];
            spans.extend(
                code_spans(line, app.code.hscroll, text_w, theme)
                    .into_iter()
                    .map(|span| Span::styled(span.content, span.style.add_modifier(outside))),
            );
            let mut out = fill_line(
                Line::from(spans).style(theme.mark_bg(line.mark)),
                usize::from(text_area.width),
            );
            if is_cursor {
                out = out.style(theme.selected());
            } else if selection.is_some_and(|(a, b)| (a..=b).contains(&i)) {
                out = out.style(theme.selection());
            }
            out
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), text_area);
    scrollbar(frame, text_area, view.lines.len(), app.code.offset, theme);

    if footer_h > 0 {
        let first = &at_cursor[0];
        let more = if at_cursor.len() > 1 {
            t::code_more_comments(l, at_cursor.len() - 1)
        } else {
            String::new()
        };
        let width = usize::from(footer_area.width);
        let text = vec![
            Line::from(vec![
                Span::styled("▌ ", theme.warn()),
                Span::styled(
                    format!("● {}{more}", fit(&first.0, width.saturating_sub(14))),
                    theme.warn(),
                ),
            ]),
            Line::from(vec![
                Span::styled("▌ ", theme.warn()),
                Span::styled(fit(&first.1, width.saturating_sub(2)), theme.dim()),
            ]),
        ];
        frame.render_widget(Paragraph::new(text), footer_area);
    }
}

/// Lines of the shown version that carry a comment, and the comments on the cursor line as
/// `(status and severity, text)`.
fn comment_lines(app: &App, view: &crate::app::CodeView) -> (HashSet<u32>, Vec<(String, String)>) {
    let mut lines = HashSet::new();
    let mut at_cursor = Vec::new();
    let Some(unit) = &view.unit else {
        return (lines, at_cursor);
    };
    let cursor = view.lines.get(app.code.cursor).map(|l| l.number);
    for comment in app.session.comments_on(unit) {
        if comment.anchor.version != view.version {
            continue;
        }
        let range = comment
            .anchor
            .line_range
            .or_else(|| view.symbol_lines.map(|(s, _)| (s, s)));
        let Some((from, to)) = range else { continue };
        lines.extend(from..=to);
        if cursor.is_some_and(|c| (from..=to).contains(&c)) {
            let severity = comment
                .severity
                .map_or_else(String::new, |s| format!("{} · ", s.label()));
            at_cursor.push((
                format!(
                    "{severity}{}",
                    labels::comment_status(app.locale, comment.status)
                ),
                clean(&comment.text),
            ));
        }
    }
    // Put the text first: the footer shows `status` on the first line and `text` below.
    let at_cursor = at_cursor.into_iter().collect();
    (lines, at_cursor)
}

// ---- symbol panel ---------------------------------------------------------------------------------------------

/// Makes room for the symbol panel at the right of the code when the user wants it and the pane is
/// wide enough, and remembers both facts for the key that toggles it.
fn split_inspector(app: &mut App, body: Rect) -> (Rect, Option<Rect>) {
    let fits = body.width >= 80;
    app.inspector_fits = fits;
    // On its own the panel opens only where the code keeps a comfortable width beside it.
    let open = fits && app.inspector.unwrap_or(body.width >= 110);
    app.inspector_shown = open;
    if !open {
        return (body, None);
    }
    let width = (body.width * 2 / 5).clamp(38, 60);
    let gap = 3;
    let code = Rect {
        width: body.width - width - gap,
        ..body
    };
    let side = Rect {
        x: body.x + code.width + gap,
        width,
        ..body
    };
    (code, Some(side))
}

/// The panel beside the code: what the symbol under review is (its signature and types), what its
/// author wrote about it, what the reviewer wrote about it, and why it has its priority.
fn draw_inspector(frame: &mut Frame<'_>, area: Rect, app: &App, view: &crate::app::CodeView) {
    let theme = app.theme;
    vline(
        frame,
        Rect {
            x: area.x.saturating_sub(2),
            ..area
        },
        theme,
    );
    let width = usize::from(area.width);
    let mut lines = inspector_lines(app, view, width);
    let room = usize::from(area.height);
    if lines.len() > room && room > 0 {
        lines.truncate(room - 1);
        lines.push(Line::from(Span::styled("…", theme.faint())));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// A section title, capitalised, after a blank line.
fn section(title: &str, _width: usize, theme: Theme) -> Vec<Line<'static>> {
    let mut chars = title.chars();
    let title: String = chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
    vec![
        Line::from(""),
        Line::from(Span::styled(title, theme.accent())),
    ]
}

fn inspector_lines(app: &App, view: &crate::app::CodeView, width: usize) -> Vec<Line<'static>> {
    let theme = app.theme;
    let l = app.locale;
    let delta = view
        .unit
        .as_ref()
        .and_then(|unit| app.session.contract(unit));
    let item = app.current_item();
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Who: the name, and what kind of change this is.
    let name = delta.as_ref().map_or_else(
        || item.map_or_else(String::new, |i| clean(&i.unit.label)),
        |d| clean(&d.name),
    );
    lines.push(Line::from(Span::styled(fit(&name, width), theme.strong())));
    if let Some(d) = &delta {
        let mut facts = vec![labels::change_kind(l, d.change_kind).to_owned()];
        if let Some(v) = d.visibility_after.as_ref().or(d.visibility_before.as_ref()) {
            facts.push(clean(v));
        }
        lines.push(Line::from(Span::styled(
            fit(&facts.join(" · "), width),
            theme.dim(),
        )));
    }

    // The signature, with its types.
    if let Some(d) = &delta {
        let signature = signature_lines(d, width, app);
        if !signature.is_empty() {
            lines.extend(section(t::inspector_signature(l), width, theme));
            lines.extend(signature);
        }
    }

    // What the author wrote.
    lines.extend(section(t::inspector_doc(l), width, theme));
    match &view.doc {
        Some(doc) => {
            for paragraph in doc.split('\n') {
                if paragraph.trim().is_empty() {
                    lines.push(Line::from(""));
                    continue;
                }
                for chunk in wrap(&clean(paragraph), width) {
                    lines.push(Line::from(Span::styled(chunk, theme.text())));
                }
            }
        }
        None => lines.push(Line::from(Span::styled(
            fit(t::inspector_no_doc(l), width),
            theme.faint(),
        ))),
    }

    // What else the declaration says: decorators, routes, errors, side effects.
    if let Some(d) = &delta {
        for (label, change) in [
            (t::contract_decorators(l), &d.decorators),
            (t::contract_endpoints(l), &d.endpoints),
            (t::contract_raises(l), &d.raises),
            (t::contract_effects(l), &d.effects),
        ] {
            if change.before.is_empty() && change.after.is_empty() {
                continue;
            }
            lines.extend(section(label, width, theme));
            lines.extend(delta_lines(change, width, theme));
        }
    }

    // What the reviewer wrote.
    let comments = view
        .unit
        .as_ref()
        .map(|unit| app.session.comments_on(unit))
        .unwrap_or_default();
    lines.extend(section(t::inspector_comments(l), width, theme));
    if comments.is_empty() {
        lines.push(Line::from(Span::styled(
            fit(t::inspector_no_comments(l), width),
            theme.faint(),
        )));
    }
    for comment in comments.iter().take(6) {
        lines.push(Line::from(vec![
            Span::styled("● ", theme.warn()),
            Span::styled(
                labels::comment_status(l, comment.status).to_owned(),
                comment_status_style(comment.status, theme),
            ),
        ]));
        for chunk in wrap(&clean(&comment.text), width.saturating_sub(2)) {
            lines.push(Line::from(format!("  {chunk}")));
        }
    }

    // Why it has its priority, and the findings behind it.
    if let Some(item) = item {
        lines.extend(section(t::explain_why(l), width, theme));
        // `explain_lines` opens with the name and the score, which the panel already shows, and
        // then its own title; the reasons are what follows that title.
        let all = explain_lines(app, item, width);
        let after_title = all
            .iter()
            .position(|line| line.spans.iter().any(|s| s.content == t::explain_why(l)))
            .map_or(0, |at| at + 1);
        lines.extend(all.into_iter().skip(after_title));
    }
    lines
}

/// The signature as a typed parameter list: what each parameter is called and what type it has,
/// marked `+` when the change added it and `−` when it removed it.
fn signature_lines(
    delta: &crate::app::ContractDelta,
    width: usize,
    app: &App,
) -> Vec<Line<'static>> {
    let theme = app.theme;
    let l = app.locale;
    let short = clean(delta.name.rsplit('.').next().unwrap_or(&delta.name));
    let Some(after) = &delta.signature_after else {
        // Not a callable (a class, a constant...): the declaration as written.
        return match (&delta.after_signature, &delta.before_signature) {
            (Some(text), _) => wrap(&clean(text), width)
                .into_iter()
                .map(|chunk| Line::from(Span::styled(chunk, theme.text())))
                .collect(),
            (None, Some(old)) => wrap(&clean(old), width)
                .into_iter()
                .map(|chunk| Line::from(Span::styled(chunk, theme.faint())))
                .collect(),
            (None, None) => Vec::new(),
        };
    };
    let before = delta.signature_before.as_ref();
    let old_params: &[String] = before.map_or(&[], |p| &p.parameters);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            short,
            theme.syntax(crate::domain::symbol::HighlightClass::Function),
        ),
        Span::styled("(", theme.dim()),
    ])];
    let name_of = |param: &str| -> String {
        param
            .split([':', '='])
            .next()
            .unwrap_or(param)
            .trim()
            .to_owned()
    };
    let old_by_name: Vec<(String, &String)> = old_params.iter().map(|p| (name_of(p), p)).collect();
    for param in &after.parameters {
        let previous = old_by_name.iter().find(|(name, _)| *name == name_of(param));
        let (mark, trail) = match (before.is_some(), previous) {
            (false, _) => (Span::raw("  "), None),
            (true, None) => (Span::styled("+ ", theme.good()), None),
            (true, Some((_, old))) if *old == param => (Span::raw("  "), None),
            (true, Some((_, old))) => (Span::styled("~ ", theme.warn()), Some((*old).clone())),
        };
        let mut spans = vec![Span::raw("  "), mark];
        spans.extend(param_spans(&clean(param), theme));
        spans.push(Span::styled(",", theme.faint()));
        if let Some(old) = trail {
            spans.push(Span::styled(
                fit(&format!("  ← {}", clean(&old)), width.saturating_sub(24)),
                theme.faint(),
            ));
        }
        lines.push(Line::from(spans));
    }
    let new_names: Vec<String> = after.parameters.iter().map(|p| name_of(p)).collect();
    for param in old_params
        .iter()
        .filter(|p| !new_names.contains(&name_of(p)))
    {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("− ", theme.error()),
            Span::styled(clean(param), theme.faint()),
        ]));
    }
    let mut close = vec![Span::styled(")", theme.dim())];
    if let Some(returns) = &after.returns {
        close.push(Span::styled(" → ", theme.faint()));
        close.push(Span::styled(
            clean(returns),
            theme.syntax(crate::domain::symbol::HighlightClass::Type),
        ));
    }
    lines.push(Line::from(close));
    if let (Some(old), Some(new)) = (before.and_then(|p| p.returns.as_ref()), &after.returns)
        && old != new
    {
        lines.push(Line::from(Span::styled(
            fit(
                &format!("  {} → {}", t::inspector_before(l), clean(old)),
                width,
            ),
            theme.faint(),
        )));
    }
    lines
}

/// One parameter, `name: Type` or `name=default`, with the name, the type and the default apart.
fn param_spans(param: &str, theme: Theme) -> Vec<Span<'static>> {
    use crate::domain::symbol::HighlightClass;
    if let Some((name, ty)) = param.split_once(':') {
        let (ty, default) = ty.split_once('=').map_or((ty, None), |(t, d)| (t, Some(d)));
        let mut spans = vec![
            Span::styled(name.trim().to_owned(), theme.strong()),
            Span::styled(": ", theme.faint()),
            Span::styled(ty.trim().to_owned(), theme.syntax(HighlightClass::Type)),
        ];
        if let Some(default) = default {
            spans.push(Span::styled(" = ", theme.faint()));
            spans.push(Span::styled(
                default.trim().to_owned(),
                theme.syntax(HighlightClass::Number),
            ));
        }
        return spans;
    }
    if let Some((name, default)) = param.split_once('=') {
        return vec![
            Span::styled(name.trim().to_owned(), theme.strong()),
            Span::styled(" = ", theme.faint()),
            Span::styled(
                default.trim().to_owned(),
                theme.syntax(HighlightClass::Number),
            ),
        ];
    }
    vec![Span::styled(param.to_owned(), theme.strong())]
}

/// The entries of a before/after list, `+` for new ones and `−` for dropped ones.
fn delta_lines(change: &crate::app::Delta, width: usize, theme: Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for item in &change.after {
        let added = !change.before.is_empty() && !change.before.contains(item);
        let mark = if added {
            Span::styled("+ ", theme.good())
        } else {
            Span::raw("  ")
        };
        lines.push(Line::from(vec![
            mark,
            Span::raw(fit(&clean(item), width.saturating_sub(2))),
        ]));
    }
    for item in change.before.iter().filter(|i| !change.after.contains(i)) {
        lines.push(Line::from(vec![
            Span::styled("− ", theme.error()),
            Span::styled(fit(&clean(item), width.saturating_sub(2)), theme.faint()),
        ]));
    }
    lines
}

/// Syntax-coloured spans for one line, after horizontal scrolling and cut to `width` columns.
fn code_spans(line: &CodeLine, hscroll: usize, width: usize, theme: Theme) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    let mut current = String::new();
    let mut current_style = Style::default();
    let flush = |spans: &mut Vec<Span<'static>>, text: &mut String, style: Style| {
        if !text.is_empty() {
            spans.push(Span::styled(std::mem::take(text), style));
        }
    };
    for (index, c) in line.text.chars().enumerate().skip(hscroll) {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        let style = line
            .spans
            .iter()
            .find(|s| (s.start..s.end).contains(&index))
            .map_or_else(|| theme.mark_text(line.mark), |s| theme.syntax(s.class));
        if style != current_style {
            flush(&mut spans, &mut current, current_style);
            current_style = style;
        }
        current.push(c);
        used += w;
    }
    flush(&mut spans, &mut current, current_style);
    if hscroll > 0 && line.text.chars().count() > hscroll + 1 && spans.is_empty() {
        spans.push(Span::styled("…", theme.dim()));
    }
    spans
}

// ---- impact ------------------------------------------------------------------------------------------------------

fn impact(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let (name, place) = app.impact_subject().map_or_else(
        || (t::impact_no_subject(l).to_owned(), String::new()),
        |k| (clean(&k.qualified_name), clean(k.file.as_ref())),
    );
    let body = panel(frame, area, &t::impact_title(l, &name), &place, theme);

    // The answer in one glance, above the lists it summarises.
    let mut head: Vec<Line<'static>> = Vec::new();
    if !app.impact.summary.is_empty() {
        head.push(Line::from(Span::styled(
            fit(&clean(&app.impact.summary), usize::from(body.width)),
            theme.strong(),
        )));
        head.push(Line::from(Span::styled(
            fit(&clean(&app.impact.evidence), usize::from(body.width)),
            theme.dim(),
        )));
        head.push(Line::from(""));
    }
    let head_h = u16::try_from(head.len()).unwrap_or(0).min(body.height / 2);
    frame.render_widget(
        Paragraph::new(head),
        Rect {
            height: head_h,
            ..body
        },
    );
    let body = Rect {
        y: body.y + head_h,
        height: body.height - head_h,
        ..body
    };

    let width = usize::from(body.width).saturating_sub(2);
    let lines = stride(app, 2, 3);
    let after_note: Vec<bool> = std::iter::once(false)
        .chain(
            app.impact
                .rows
                .iter()
                .map(|row| row.kind == ImpactRowKind::Note),
        )
        .collect();
    let cards: Vec<Vec<Line<'static>>> = app
        .impact
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| match row.kind {
            ImpactRowKind::Heading => {
                let rule = Line::from(Span::styled("─".repeat(width), theme.border()));
                let title = Line::from(vec![
                    Span::styled(row.text.clone(), theme.accent()),
                    Span::styled(format!("  {}", row.detail), theme.faint()),
                ]);
                // After a note the air is already there.
                let air = if after_note[index] {
                    Vec::new()
                } else {
                    vec![Line::from("")]
                };
                let mut card = air;
                card.push(title);
                if lines >= 3 {
                    card.push(rule);
                }
                card
            }
            ImpactRowKind::Note => vec![Line::from(Span::styled(
                format!("  {}", fit(&row.text, width.saturating_sub(2))),
                theme.dim(),
            ))],
            ImpactRowKind::Entrypoint | ImpactRowKind::Caller | ImpactRowKind::Callee => {
                impact_card(l, theme, row, width)
            }
        })
        .collect();
    draw_cards(frame, body, &mut app.impact_view, cards, lines, theme);
}

/// A symbol in the impact view: its name and how sure the link is, then where it lives.
fn impact_card(
    l: crate::domain::i18n::Locale,
    theme: Theme,
    row: &crate::tui::app::ImpactRow,
    width: usize,
) -> Vec<Line<'static>> {
    let indent = " ".repeat(2 + row.depth * 2);
    let (mark, mark_style) = match row.origin {
        Some(crate::domain::graph::EdgeOrigin::Resolved) => ("●", theme.good()),
        Some(crate::domain::graph::EdgeOrigin::Heuristic) => ("◐", theme.warn()),
        _ => ("○", theme.dim()),
    };
    let origin = row.origin.map_or("", |o| labels::origin(l, o));
    let lead = indent.width() + 2;
    let name_w = width.saturating_sub(lead + origin.width() + 2);
    let top = Line::from(vec![
        Span::raw(indent.clone()),
        Span::styled(format!("{mark} "), mark_style),
        Span::styled(pad(&fit(&row.text, name_w), name_w), theme.strong()),
        Span::styled(format!("  {origin}"), mark_style),
    ]);
    let room = width.saturating_sub(lead);
    let place = Line::from(vec![
        Span::raw(" ".repeat(lead)),
        Span::styled(fit_tail(&row.detail, room), theme.faint()),
    ]);
    let mut card = vec![top, place];
    if !row.extra.is_empty() {
        card.push(Line::from(vec![
            Span::raw(" ".repeat(lead)),
            Span::styled(fit(&format!("→ {}", row.extra), room), theme.dim()),
        ]));
    }
    card
}

// ---- comments -------------------------------------------------------------------------------------------------------

fn comments(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let total = app.session.review().comments.len();
    let body = panel(
        frame,
        area,
        &t::comments_title(l, total),
        &t::comments_hint(
            l,
            app.keymap.key_for("edit_comment"),
            app.keymap.key_for("delete_comment"),
        ),
        theme,
    );
    if total == 0 {
        empty_state(frame, body, t::comments_empty(l), theme);
        return;
    }
    let detail_h: u16 = if body.height >= 14 { 6 } else { 0 };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(detail_h)])
        .split(body);
    let width = usize::from(parts[0].width);
    let rows: Vec<Line<'static>> = app
        .session
        .review()
        .comments
        .iter()
        .map(|c| {
            let place = c
                .anchor
                .line_range
                .map_or_else(String::new, |(a, _)| format!(":{a}"));
            let status = labels::comment_status(l, c.status);
            let warn = if matches!(c.status, CommentStatus::Stale) {
                "⚠ "
            } else {
                ""
            };
            let severity = c.severity.map_or_else(
                || Span::raw("  "),
                |s| Span::styled(s.label().to_owned(), theme.priority(s)),
            );
            let file = format!(" {}{place} ", clean(c.anchor.file_path.as_str()));
            let left_w = width / 2;
            let fixed = warn.width() + 10 + 2;
            Line::from(vec![
                Span::styled(warn, theme.warn()),
                Span::styled(pad(status, 9), comment_status_style(c.status, theme)),
                Span::raw(" "),
                severity,
                Span::styled(fit(&file, left_w.saturating_sub(fixed)), theme.faint()),
                Span::raw(fit(
                    &clean(&c.text),
                    width.saturating_sub(left_w.max(fixed) + 1),
                )),
            ])
        })
        .collect();
    draw_list(frame, parts[0], &mut app.comments_view, rows, theme);
    if detail_h > 0
        && let Some(detail) = Some(parts[1])
    {
        rule(frame, detail, theme);
    }
    if detail_h > 0
        && let Some(c) = app
            .session
            .review()
            .comments
            .get(app.comments_view.selected)
    {
        let mut lines = vec![Line::from(Span::styled(
            t::comment_detail(
                l,
                labels::comment_status(l, c.status),
                &format!("{:.0}", c.anchor_confidence.value() * 100.0),
                &c.anchor
                    .qualified_name
                    .as_deref()
                    .map_or_else(|| t::comment_target_file(l).to_owned(), clean),
            ),
            theme.dim(),
        ))];
        for chunk in wrap(
            &clean_multiline(&c.text),
            usize::from(parts[1].width).saturating_sub(2),
        )
        .into_iter()
        .take(4)
        {
            lines.push(Line::from(chunk));
        }
        let below_rule = Rect {
            y: parts[1].y + 1,
            height: parts[1].height.saturating_sub(1),
            ..parts[1]
        };
        frame.render_widget(Paragraph::new(lines), inset(below_rule, 1));
    }
}

/// Comment text keeps its line breaks but loses terminal sequences.
fn clean_multiline(text: &str) -> String {
    crate::domain::text::sanitize_terminal_text(text)
}

// ---- context ------------------------------------------------------------------------------------------------------------

fn context(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let body = panel(frame, area, t::mode_context(l), t::context_hint(l), theme);
    let brief = app.session.brief().clone();
    let width = usize::from(body.width);
    let mut rows: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled(t::context_intent(l), theme.accent()),
        Span::raw(fit(
            &brief
                .intent_summary
                .as_deref()
                .map_or_else(|| t::context_intent_unset(l).to_owned(), clean),
            width.saturating_sub(10),
        )),
    ])];
    for criterion in &brief.acceptance_criteria {
        rows.push(Line::from(vec![
            Span::styled(
                format!("{} ", labels::criterion_mark(criterion.status)),
                match criterion.status {
                    CriterionStatus::Verified => theme.good(),
                    CriterionStatus::Pending => theme.warn(),
                    CriterionStatus::NotChecked | CriterionStatus::NotApplicable => theme.dim(),
                },
            ),
            Span::raw(fit(&clean(&criterion.text), width.saturating_sub(18))),
            Span::styled(
                format!("  {}", labels::criterion_status(l, criterion.status)),
                theme.dim(),
            ),
        ]));
    }
    let detail_h: u16 = if body.height >= 12 { 5 } else { 0 };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(detail_h)])
        .split(body);
    draw_list(frame, parts[0], &mut app.context_view, rows, theme);
    if detail_h > 0 {
        rule(frame, parts[1], theme);
        let mut lines = Vec::new();
        let selected = app.context_view.selected;
        let text = if selected == 0 {
            brief.intent_summary.clone().unwrap_or_default()
        } else {
            brief
                .acceptance_criteria
                .get(selected - 1)
                .map(|c| c.text.clone())
                .unwrap_or_default()
        };
        for chunk in wrap(
            &clean_multiline(&text),
            usize::from(parts[1].width).saturating_sub(2),
        )
        .into_iter()
        .take(3)
        {
            lines.push(Line::from(chunk));
        }
        lines.push(Line::from(Span::styled(
            t::context_untrusted(l),
            theme.faint(),
        )));
        let below_rule = Rect {
            y: parts[1].y + 1,
            height: parts[1].height.saturating_sub(1),
            ..parts[1]
        };
        frame.render_widget(Paragraph::new(lines), inset(below_rule, 1));
    }
}

// ---- tests -------------------------------------------------------------------------------------------------------------------

fn tests(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let Some(unit) = app.unit.clone() else {
        let body = panel(frame, area, t::mode_tests(l), "", theme);
        empty_state(frame, body, t::tests_select_unit(l), theme);
        return;
    };
    let label = app
        .current_item()
        .map_or_else(String::new, |i| clean(&i.unit.label));
    let body = panel(
        frame,
        area,
        &t::tests_title(l, &label),
        t::tests_hint(l),
        theme,
    );
    let body = inset(body, 1);
    let panel = app.session.test_panel(&unit);
    let width = usize::from(body.width).saturating_sub(2);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let status_chip = match panel.status {
        EvidenceStatus::Passed => Chip::Good,
        EvidenceStatus::Failed => Chip::Bad,
        EvidenceStatus::Stale | EvidenceStatus::NoAssociationFound => Chip::Warn,
        EvidenceStatus::NotRun => Chip::Quiet,
    };
    lines.push(Line::from(vec![
        Span::raw(t::tests_evidence(l)),
        chip_span(
            labels::evidence_status(l, panel.status),
            theme.chip(status_chip),
        ),
    ]));
    let explanation = match panel.status {
        EvidenceStatus::NoAssociationFound => t::tests_none_linked(l),
        EvidenceStatus::NotRun => t::tests_not_run(l),
        EvidenceStatus::Passed => t::tests_passed(l),
        EvidenceStatus::Failed => t::tests_failed(l),
        EvidenceStatus::Stale => t::tests_stale(l),
    };
    for chunk in wrap(explanation, width) {
        lines.push(Line::from(Span::styled(chunk, theme.dim())));
    }
    if !panel.links.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            t::tests_associated(l),
            theme.accent(),
        )));
        for link in &panel.links {
            lines.push(Line::from(t::tests_link(
                l,
                &clean(&link.test.name),
                &clean(link.test.file.as_str()),
                link.test.line,
                labels::link_method(l, link.method),
                &format!("{:.0}", link.confidence.value() * 100.0),
            )));
        }
    }
    if !panel.results.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            t::tests_imported(l),
            theme.accent(),
        )));
        for result in &panel.results {
            let style = if matches!(result.outcome, crate::domain::testing::TestOutcome::Failed) {
                theme.error()
            } else {
                theme.text()
            };
            lines.push(Line::from(Span::styled(
                t::tests_result(l, &clean(&result.name), labels::outcome(l, result.outcome)),
                style,
            )));
            if let Some(message) = &result.message {
                lines.push(Line::from(Span::styled(
                    format!("      {}", fit(&clean(message), width.saturating_sub(6))),
                    theme.dim(),
                )));
            }
        }
    }
    if let Some(coverage) = &panel.coverage {
        lines.push(Line::raw(""));
        lines.push(Line::from(t::tests_coverage(
            l,
            &clean(coverage.file.as_str()),
            coverage.covered_lines.len(),
            coverage.missed_lines.len(),
        )));
    }
    lines.push(Line::raw(""));
    if panel.sources.is_empty() {
        lines.push(Line::from(Span::styled(t::tests_no_report(l), theme.dim())));
    }
    for source in &panel.sources {
        let stale = if source.stale {
            t::tests_stale_suffix(l)
        } else {
            ""
        };
        lines.push(Line::from(Span::styled(
            t::tests_report(l, &clean(&source.path), stale),
            if source.stale {
                theme.warn()
            } else {
                theme.dim()
            },
        )));
    }
    for note in &app.session.reports().notes {
        lines.push(Line::from(Span::styled(
            fit(&clean(note), width),
            theme.warn(),
        )));
    }
    if let Some(output) = &app.test_output {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(t::tests_output(l), theme.accent())));
        for line in output
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            lines.push(Line::from(Span::styled(fit(line, width), theme.dim())));
        }
    }
    let height = usize::from(body.height);
    app.tests_view.height = height;
    let offset = app
        .tests_view
        .selected
        .min(lines.len().saturating_sub(height));
    let visible: Vec<Line<'static>> = lines.into_iter().skip(offset).take(height).collect();
    frame.render_widget(Paragraph::new(visible), body);
}

// ---- follow-up ---------------------------------------------------------------------------------------------------------------------

fn followup(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let comments: Vec<_> = app.followups().into_iter().cloned().collect();
    let body = panel(
        frame,
        area,
        &t::followup_title(l, comments.len()),
        t::followup_hint(l),
        theme,
    );
    if comments.is_empty() {
        empty_state(frame, body, t::followup_empty(l), theme);
        return;
    }
    let width = usize::from(body.width);
    let rows: Vec<Line<'static>> = comments
        .iter()
        .map(|c| {
            let place = c
                .anchor
                .line_range
                .map_or_else(String::new, |(a, _)| format!(":{a}"));
            let confidence = format!("{:.0}%", c.anchor_confidence.value() * 100.0);
            let status_style = match c.followup_status {
                FollowupStatus::Resolved => theme.good(),
                FollowupStatus::Stale | FollowupStatus::Regression => theme.error(),
                FollowupStatus::Partial | FollowupStatus::Unresolved => theme.warn(),
                FollowupStatus::Pending => theme.dim(),
            };
            let left_w = width / 2;
            let tail = format!(
                " {} {}{place} ({confidence}) ",
                pad(labels::comment_status(l, c.status), 9),
                clean(c.anchor.file_path.as_str())
            );
            Line::from(vec![
                Span::styled(
                    pad(labels::followup_status(l, c.followup_status), 10),
                    status_style,
                ),
                Span::styled(fit(&tail, left_w.saturating_sub(10)), theme.faint()),
                Span::raw(fit(
                    &clean(&c.text),
                    width.saturating_sub(left_w.max(10) + 1),
                )),
            ])
        })
        .collect();
    draw_list(frame, body, &mut app.followup_view, rows, theme);
}

// ---- coverage ---------------------------------------------------------------------------------------------------------------------------

fn coverage(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let cov = app.session.coverage();
    let body = panel(frame, area, t::mode_coverage(l), t::coverage_hint(l), theme);
    let width = usize::from(body.width).saturating_sub(2);
    let bar_w = (width / 4).clamp(6, 30);
    let gauge_spans = |done: usize, all: usize, fill: Style| -> Vec<Span<'static>> {
        let (filled, track) = gauge(done, all, bar_w);
        vec![
            Span::styled(filled, fill),
            Span::styled(track, theme.border()),
        ]
    };
    let overall = percent(cov.reviewed, cov.total);
    let mut overall_line = vec![Span::styled(t::coverage_overall(l), theme.accent())];
    overall_line.extend(gauge_spans(cov.reviewed, cov.total, theme.gauge(overall)));
    overall_line.push(Span::raw(format!(
        " {}/{} ({}%)",
        cov.reviewed, cov.total, overall
    )));
    let mut top: Vec<Line<'static>> = vec![Line::from(overall_line)];
    for (priority, (done, all)) in &cov.by_priority {
        let mut spans = vec![Span::styled(
            format!("  {}     ", priority.label()),
            theme.priority(*priority),
        )];
        spans.extend(gauge_spans(*done, *all, theme.priority(*priority)));
        spans.push(Span::raw(format!(" {done}/{all}")));
        top.push(Line::from(spans));
    }
    if cov.unknown > 0 {
        top.push(Line::from(Span::styled(
            t::coverage_unknown(l, cov.unknown),
            theme.warn(),
        )));
    }
    if !cov.pending_critical.is_empty() {
        top.push(Line::from(Span::styled(
            t::coverage_pending_critical(l, cov.pending_critical.len()),
            theme.warn(),
        )));
    }
    let notes = app.session.analysis().notes.clone();
    let top_h = u16::try_from(top.len())
        .unwrap_or(u16::MAX)
        .min(body.height / 2);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(top_h), Constraint::Min(2)])
        .split(body);
    frame.render_widget(Paragraph::new(top), inset(parts[0], 1));
    let mut rows: Vec<Line<'static>> = cov
        .by_module
        .iter()
        .map(|(module, (done, all))| {
            let mut spans = vec![
                Span::raw(pad(&clean(module), (width / 3).clamp(12, 40))),
                Span::raw(" "),
            ];
            spans.extend(gauge_spans(*done, *all, theme.gauge(percent(*done, *all))));
            spans.push(Span::raw(format!(" {done}/{all}")));
            Line::from(spans)
        })
        .collect();
    for note in &notes {
        rows.push(Line::from(Span::styled(
            t::coverage_notice(l, &fit(&clean(note), width.saturating_sub(12))),
            theme.dim(),
        )));
    }
    draw_list(frame, parts[1], &mut app.coverage_view, rows, theme);
}

// ---- flow ----------------------------------------------------------------------------------------------------------------------------------

fn flow(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let l = app.locale;
    let view = app.session.flow().clone();
    let state_label = labels::flow_state(l, &view.state);
    let body = panel(
        frame,
        area,
        &t::flow_title(l, state_label),
        t::flow_hint(l),
        theme,
    );
    let width = usize::from(body.width).saturating_sub(4);
    let mut lines: Vec<Line<'static>> = Vec::new();
    match (&view.state, &view.flow) {
        (FlowState::Missing, _) => {
            lines.push(Line::from(Span::styled(t::flow_not_found(l), theme.warn())));
            lines.push(Line::from(Span::styled(
                t::flow_expected(l, &clean(view.expected_path.as_str())),
                theme.dim(),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from(t::flow_press_p(l)));
            lines.push(Line::from(t::flow_works_without(l)));
            lines.push(Line::raw(""));
            for chunk in wrap(&clean(&view.request_prompt), width) {
                lines.push(Line::from(Span::styled(chunk, theme.dim())));
            }
        }
        (_, None) => {
            lines.push(Line::from(Span::styled(t::flow_unusable(l), theme.error())));
            for issue in &view.issues {
                for chunk in wrap(&clean(issue), width) {
                    lines.push(Line::from(format!("  {chunk}")));
                }
            }
            lines.push(Line::from(Span::styled(t::flow_fix(l), theme.dim())));
        }
        (state, Some(validated)) => {
            if let FlowState::Stale { found } = state {
                lines.push(Line::from(Span::styled(
                    t::flow_snapshot_mismatch(l, &clean(found)),
                    theme.warn(),
                )));
            }
            lines.push(Line::from(Span::styled(
                clean(&validated.document.title),
                theme.strong(),
            )));
            for chunk in wrap(&clean(&validated.document.summary), width) {
                lines.push(Line::from(chunk));
            }
            lines.push(Line::from(Span::styled(
                t::flow_written_by(l, &clean(&validated.document.generated_by)),
                theme.dim(),
            )));
            for divergence in &validated.divergences {
                lines.push(Line::from(Span::styled(
                    format!("⚠ {}", fit(&clean(divergence), width.saturating_sub(2))),
                    theme.warn(),
                )));
            }
            lines.push(Line::raw(""));
        }
    }
    // Steps are a list the reviewer moves through; text above scrolls with it.
    let Some(validated) = &view.flow else {
        frame.render_widget(Paragraph::new(lines), inset(body, 1));
        return;
    };
    let intro_h = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(body.height.saturating_sub(3));
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(intro_h), Constraint::Min(2)])
        .split(body);
    frame.render_widget(Paragraph::new(lines), inset(parts[0], 1));
    let lines = app.flow_rows();
    let list_width = usize::from(parts[1].width);
    let [crumbs_area, list_area] = {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(parts[1]);
        [split[0], split[1]]
    };
    let step_name = |index: usize| -> String {
        validated.document.steps.get(index).map_or_else(
            || "?".to_owned(),
            |step| {
                step.symbol
                    .as_deref()
                    .or(step.file_path.as_deref())
                    .unwrap_or(&step.id)
                    .to_owned()
            },
        )
    };
    let crumbs = app
        .flow_breadcrumbs()
        .into_iter()
        .map(|i| clean(&step_name(i)))
        .collect::<Vec<_>>()
        .join(" › ");
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            fit_tail(&t::flow_path(l, &crumbs), list_width),
            theme.dim(),
        ))),
        crumbs_area,
    );
    let rows: Vec<Line<'static>> = lines
        .iter()
        .map(|line| {
            let step = &validated.document.steps[line.row.step];
            let resolved = validated.steps.get(line.row.step);
            let place = match (&step.file_path, &step.symbol) {
                (Some(f), Some(s)) => format!("{}::{}", clean(f), clean(s)),
                (Some(f), None) => clean(f),
                _ => step.external_ref.as_ref().map_or_else(
                    || t::flow_external(l).to_owned(),
                    |e| clean(&format!("{e:?}")),
                ),
            };
            let missing = if resolved
                .is_some_and(|r| r.file.is_some() && !r.symbol_found && r.symbol.is_some())
            {
                t::flow_symbol_missing(l)
            } else {
                ""
            };
            let changed = if step.changed { t::flow_changed(l) } else { "" };
            let fold = if line.folded {
                "▸"
            } else if line.has_branches {
                "▾"
            } else {
                " "
            };
            let via = line.row.via.map_or_else(String::new, |(kind, origin)| {
                format!(
                    "{}{} ",
                    labels::edge_kind(l, kind),
                    labels::origin_mark(origin)
                )
            });
            let again = if line.row.revisit {
                t::flow_shown_above(l)
            } else {
                ""
            };
            let indent = "  ".repeat(line.row.depth.min(8));
            Line::from(vec![
                Span::raw(format!(
                    "{:>2} {} {fold} {indent}",
                    line.row.step + 1,
                    labels::origin_mark(step.confidence)
                )),
                Span::styled(via, theme.dim()),
                Span::raw(fit(
                    &format!("{place}{changed}{missing}{again}"),
                    list_width.saturating_sub(12 + indent.len()),
                )),
            ])
        })
        .collect();
    draw_list(frame, list_area, &mut app.flow_view, rows, theme);
}

/// Whether a line mark carries a text colour of its own (used by the code view).
#[allow(dead_code)]
fn is_changed(mark: LineMark) -> bool {
    mark != LineMark::Same
}
