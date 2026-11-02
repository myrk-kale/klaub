//! The main area for each mode. Everything that comes from the repository, a comment or a document
//! goes through [`clean`] before it reaches the screen.

use std::collections::HashSet;

use crate::app::CodeLine;
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
    chip_span, clean, draw_list, empty_state, fill_line, fit, fit_tail, gauge, inset, pad, panel,
    panel_with, percent, scrollbar, wrap,
};

/// Draws the main area for the current mode.
pub(crate) fn draw_mode(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    match app.mode {
        Mode::Queue => queue(frame, area, app),
        Mode::Structure => structure(frame, area, app),
        Mode::Code => code(frame, area, app),
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

// ---- queue ------------------------------------------------------------------------------------------------

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
    let title = t::queue_title(l, visible, total, &summary);

    if visible == 0 {
        let body = panel(frame, area, &title, "", theme);
        let text = if total == 0 {
            t::queue_nothing_changed(l)
        } else {
            t::queue_no_match(l)
        };
        empty_state(frame, body, text, theme);
        return;
    }
    let wide = area.width >= 100;
    let (list_area, detail_area) = if wide {
        let parts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        (parts[0], Some(parts[1]))
    } else {
        (area, None)
    };
    let list_inner = panel(frame, list_area, &title, "", theme);
    let width = usize::from(list_inner.width);
    let rows: Vec<Line<'static>> = app
        .visible_queue()
        .iter()
        .map(|item| queue_row(app, item, width))
        .collect();
    draw_list(frame, list_inner, &mut app.queue_view, rows, theme);
    if let Some(detail) = detail_area {
        let inner = panel(frame, detail, t::queue_details(l), "", theme);
        let selected = app
            .queue_selection()
            .and_then(|u| app.session.analysis().item(&u));
        if let Some(item) = selected {
            let inner = inset(inner, 1);
            frame.render_widget(
                Paragraph::new(explain_lines(app, item, usize::from(inner.width))),
                inner,
            );
        }
    }
}

fn queue_row(app: &App, item: &QueueItem, width: usize) -> Line<'static> {
    let theme = app.theme;
    let l = app.locale;
    let reviewed = app.session.review().is_reviewed(&item.unit.id);
    let comments = app.session.comments_on(&item.unit.id).len();
    // One column for the selection bar, then the check, the priority chip and a space.
    let rest = width.saturating_sub(9);
    let label_w = (rest * 32 / 100).max(12);
    let kind_w = 10;
    let narrow = width < 64;
    let tags: Vec<&str> = item
        .tags
        .iter()
        .map(|tag| labels::queue_tag(l, *tag))
        .collect();
    let tags_w = if narrow { 0 } else { (rest * 22 / 100).max(8) };
    let file_w = rest.saturating_sub(label_w + kind_w + tags_w + 3);
    let label_style = if reviewed { theme.dim() } else { theme.text() };
    let mut spans = vec![
        Span::styled(if reviewed { "✓ " } else { "  " }, theme.good()),
        chip_span(item.priority.label(), theme.priority_chip(item.priority)),
        Span::raw(" "),
        Span::styled(pad(&clean(&item.unit.label), label_w), label_style),
        Span::styled(
            format!(" {}", pad(labels::change_kind(l, item.change), kind_w)),
            theme.dim(),
        ),
    ];
    if !narrow {
        spans.push(Span::styled(
            format!(" {}", pad(&tags.join(","), tags_w)),
            theme.key().remove_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        format!(" {}", fit(&clean(item.unit.file.as_str()), file_w)),
        theme.faint(),
    ));
    if comments > 0 {
        spans.push(Span::styled(format!(" ✎{comments}"), theme.warn()));
    }
    Line::from(spans)
}

/// The reasons behind a priority, one line each.
pub(crate) fn explain_lines(app: &App, item: &QueueItem, width: usize) -> Vec<Line<'static>> {
    let theme = app.theme;
    let l = app.locale;
    let mut lines = vec![Line::from(vec![
        chip_span(item.priority.label(), theme.priority_chip(item.priority)),
        Span::styled(
            t::explain_score(l, item.score.total(), &clean(&item.unit.label)),
            theme.strong(),
        ),
    ])];
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(t::explain_why(l), theme.accent())));
    for part in item.score.parts.iter().filter(|p| p.points != 0) {
        let text = format!(
            "{:+} {} — {}",
            part.points,
            labels::score_component(l, part.component),
            clean(&part.reason)
        );
        for (i, chunk) in wrap(&text, width).into_iter().enumerate() {
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
            for chunk in wrap(&text, width) {
                lines.push(Line::from(format!("  {chunk}")));
            }
            for evidence in finding.evidence.iter().take(2) {
                let place = evidence
                    .span
                    .map_or_else(String::new, |s| format!(":{}", s.start_line));
                lines.push(Line::from(Span::styled(
                    format!(
                        "    ↳ {}{place} {}",
                        clean(evidence.file.as_str()),
                        fit(&clean(&evidence.detail), width.saturating_sub(8))
                    ),
                    theme.dim(),
                )));
            }
        }
    }
    if !item.findings.is_empty() {
        lines.push(Line::from(Span::styled(
            t::explain_false_positive(l),
            theme.dim(),
        )));
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
    let body = panel(
        frame,
        area,
        t::mode_structure(l),
        t::structure_hint(l),
        theme,
    );
    let tree = app.tree_rows();
    if tree.is_empty() {
        empty_state(frame, body, t::structure_empty(l), theme);
        return;
    }
    let width = usize::from(body.width);
    let rows: Vec<Line<'static>> = tree
        .iter()
        .map(|row| {
            let glyph = match &row.kind {
                TreeKind::File(path) if app.collapsed.contains(path) => "▸ ",
                TreeKind::Unit(_) => "• ",
                TreeKind::Module(_) | TreeKind::File(_) => "▾ ",
            };
            let (done, all) = row.progress;
            let progress = if all > 1 || !matches!(row.kind, TreeKind::Unit(_)) {
                format!(" ({done}/{all})")
            } else if done == 1 {
                " ✓".to_owned()
            } else {
                String::new()
            };
            let priority = row
                .priority
                .map_or_else(|| "  ".to_owned(), |p| p.label().to_owned());
            let indent = "│ ".repeat(row.depth);
            let room = width.saturating_sub(progress.len() + 6);
            let label = fit(
                &format!("{glyph}{}", clean(&row.label)),
                room.saturating_sub(indent.width()),
            );
            let label_style = if matches!(row.kind, TreeKind::Unit(_)) {
                theme.text()
            } else {
                theme.strong()
            };
            Line::from(vec![
                Span::styled(
                    format!("{priority} "),
                    row.priority
                        .map_or_else(|| theme.dim(), |p| theme.priority(p)),
                ),
                Span::styled(indent.clone(), theme.faint()),
                Span::styled(
                    pad(&label, room.saturating_sub(indent.width())),
                    label_style,
                ),
                Span::styled(progress, theme.dim()),
            ])
        })
        .collect();
    draw_list(frame, body, &mut app.structure_view, rows, theme);
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
    let body = parts[1];

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
            let is_cursor = i == app.code.cursor;
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
    let subject = app.impact_subject().map_or_else(
        || t::impact_no_subject(l).to_owned(),
        |k| clean(&format!("{} ({})", k.qualified_name, k.file)),
    );
    let body = panel(frame, area, &t::impact_title(l, &subject), "", theme);
    let width = usize::from(body.width);
    let rows: Vec<Line<'static>> = app
        .impact
        .rows
        .iter()
        .map(|row| match row.kind {
            ImpactRowKind::Heading => {
                Line::from(Span::styled(fit(&row.text, width), theme.accent()))
            }
            ImpactRowKind::Note => Line::from(Span::styled(fit(&row.text, width), theme.dim())),
            ImpactRowKind::Entrypoint | ImpactRowKind::Caller | ImpactRowKind::Callee => {
                let mark = row.origin.map_or(' ', labels::origin_mark);
                let origin = row.origin.map_or("", |o| labels::origin(l, o));
                let text = fit(&row.text, width.saturating_sub(origin.len() + 6));
                let mark_style = match row.origin {
                    Some(crate::domain::graph::EdgeOrigin::Resolved) => theme.good(),
                    Some(crate::domain::graph::EdgeOrigin::Heuristic) => theme.warn(),
                    _ => theme.dim(),
                };
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(mark.to_string(), mark_style),
                    Span::raw(format!(" {text}")),
                    Span::styled(format!("  {origin}"), theme.faint()),
                ])
            }
        })
        .collect();
    draw_list(frame, body, &mut app.impact_view, rows, theme);
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
