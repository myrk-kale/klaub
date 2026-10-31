//! The main area for each mode. Everything that comes from the repository, a comment or a document
//! goes through [`clean`] before it reaches the screen.

use std::collections::HashSet;

use klaub_app::CodeLine;
use klaub_core::finding::{QueueItem, Temporal};
use klaub_core::flow::FlowState;
use klaub_core::review::Version;
use klaub_core::semantic::LineMark;
use klaub_core::testing::EvidenceStatus;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

use crate::app::{App, ImpactRowKind, Mode, TreeKind};
use crate::labels;
use crate::theme::{Theme, mark_symbol};
use crate::widgets::{bar, clean, draw_list, fit, fit_tail, pad, percent, wrap};

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

fn header(frame: &mut Frame<'_>, area: Rect, text: &str, theme: Theme) {
    let line = Line::from(Span::styled(
        fit(text, usize::from(area.width)),
        theme.strong(),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

fn split_header(area: Rect) -> (Rect, Rect) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    (parts[0], parts[1])
}

fn empty(frame: &mut Frame<'_>, area: Rect, text: &str, theme: Theme) {
    let lines: Vec<Line<'static>> = wrap(text, usize::from(area.width).saturating_sub(2))
        .into_iter()
        .map(|l| Line::from(Span::styled(l, theme.dim())))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

// ---- queue ------------------------------------------------------------------------------------------------

fn queue(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let total = app.session.analysis().queue.len();
    let visible = app.visible_queue().len();
    let (head, body) = split_header(area);
    let summary = if app.filters.any() {
        format!(" · filter: {}", app.filters.summary())
    } else {
        String::new()
    };
    header(
        frame,
        head,
        &format!("Queue · {visible} of {total} unit(s){summary}"),
        theme,
    );

    if visible == 0 {
        let text = if total == 0 {
            "Nothing changed relative to the base: there is nothing to review."
        } else {
            "No unit matches the filters. x clears them."
        };
        empty(frame, body, text, theme);
        return;
    }
    let wide = body.width >= 100;
    let (list_area, detail_area) = if wide {
        let parts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(body);
        (parts[0], Some(parts[1]))
    } else {
        (body, None)
    };
    let width = usize::from(list_area.width);
    let rows: Vec<Line<'static>> = app
        .visible_queue()
        .iter()
        .map(|item| queue_row(app, item, width))
        .collect();
    draw_list(frame, list_area, &mut app.queue_view, rows, theme);
    if let Some(detail) = detail_area {
        let selected = app
            .queue_selection()
            .and_then(|u| app.session.analysis().item(&u));
        if let Some(item) = selected {
            frame.render_widget(
                Paragraph::new(explain_lines(
                    app,
                    item,
                    usize::from(detail.width).saturating_sub(2),
                )),
                detail,
            );
        }
    }
}

fn queue_row(app: &App, item: &QueueItem, width: usize) -> Line<'static> {
    let theme = app.theme;
    let reviewed = app.session.review().is_reviewed(&item.unit.id);
    let comments = app.session.comments_on(&item.unit.id).len();
    let rest = width.saturating_sub(8);
    let label_w = (rest * 32 / 100).max(12);
    let kind_w = 10;
    let narrow = width < 64;
    let tags: Vec<&str> = item.tags.iter().map(|t| t.label()).collect();
    let tags_w = if narrow { 0 } else { (rest * 22 / 100).max(8) };
    let file_w = rest.saturating_sub(label_w + kind_w + tags_w + 3);
    let mut spans = vec![
        Span::styled(if reviewed { "✓ " } else { "  " }, theme.good()),
        Span::styled(
            format!("{} ", item.priority.label()),
            theme.priority(item.priority),
        ),
        Span::raw(pad(&clean(&item.unit.label), label_w)),
        Span::styled(
            format!(" {}", pad(item.change.label(), kind_w)),
            theme.dim(),
        ),
    ];
    if !narrow {
        spans.push(Span::raw(format!(" {}", pad(&tags.join(","), tags_w))));
    }
    spans.push(Span::styled(
        format!(" {}", fit(&clean(item.unit.file.as_str()), file_w)),
        theme.dim(),
    ));
    if comments > 0 {
        spans.push(Span::styled(format!(" ✎{comments}"), theme.warn()));
    }
    Line::from(spans)
}

/// The reasons behind a priority, one line each.
pub(crate) fn explain_lines(app: &App, item: &QueueItem, width: usize) -> Vec<Line<'static>> {
    let theme = app.theme;
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "{} · score {} · {}",
            item.priority.label(),
            item.score.total(),
            clean(&item.unit.label)
        ),
        theme.priority(item.priority),
    ))];
    lines.push(Line::from(Span::styled(
        "Why this priority",
        theme.strong(),
    )));
    for part in item.score.parts.iter().filter(|p| p.points != 0) {
        let text = format!(
            "{:+} {} — {}",
            part.points,
            part.component.label(),
            clean(&part.reason)
        );
        for (i, chunk) in wrap(&text, width).into_iter().enumerate() {
            lines.push(Line::from(format!(
                "{}{chunk}",
                if i == 0 { "  " } else { "      " }
            )));
        }
    }
    if item.score.parts.iter().all(|p| p.points == 0) {
        lines.push(Line::from(Span::styled(
            "  no contributing factor",
            theme.dim(),
        )));
    }
    if !item.findings.is_empty() {
        lines.push(Line::from(Span::styled("Findings", theme.strong())));
        for finding in &item.findings {
            let temporal = match finding.temporal {
                Temporal::Introduced => "introduced",
                Temporal::Preexisting => "preexisting",
                Temporal::Removed => "removed",
                Temporal::Unknown => "unknown origin",
            };
            let text = format!(
                "[{temporal}] {} ({})",
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
    if let Some(scope) = item.scope {
        lines.push(Line::from(Span::styled(
            format!(
                "Reach: {} · {} entrypoint(s)",
                scope.label(),
                item.entrypoints
            ),
            theme.dim(),
        )));
    }
    lines
}

// ---- structure -------------------------------------------------------------------------------------------

fn structure(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let (head, body) = split_header(area);
    header(
        frame,
        head,
        "Structure · modules → files → symbols (+ / - to open or close levels)",
        theme,
    );
    let tree = app.tree_rows();
    if tree.is_empty() {
        empty(
            frame,
            body,
            "Nothing to show with the current filters.",
            theme,
        );
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
            let indent = "  ".repeat(row.depth);
            let text = fit(
                &format!("{indent}{glyph}{}", clean(&row.label)),
                width.saturating_sub(progress.len() + 6),
            );
            Line::from(vec![
                Span::styled(
                    format!("{priority} "),
                    row.priority
                        .map_or_else(|| theme.dim(), |p| theme.priority(p)),
                ),
                Span::raw(pad(&text, width.saturating_sub(progress.len() + 6))),
                Span::styled(progress, theme.dim()),
            ])
        })
        .collect();
    draw_list(frame, body, &mut app.structure_view, rows, theme);
}

// ---- code ----------------------------------------------------------------------------------------------------

fn code(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let Some(view) = app.code.view.clone() else {
        empty(
            frame,
            area,
            "No code is open. Pick a unit in the Queue and press Enter.",
            theme,
        );
        return;
    };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);
    let (head, body) = (parts[0], parts[1]);

    let side = match view.version {
        Version::After => "NEW CODE",
        Version::Before => "OLD CODE",
    };
    let place = view
        .symbol_lines
        .map_or_else(String::new, |(a, b)| format!(" · lines {a}–{b}"));
    let title = format!(
        "{} · {}{place}",
        clean(view.path.as_str()),
        app.current_item()
            .map_or_else(|| "symbol".to_owned(), |i| clean(&i.unit.label)),
    );
    let width = usize::from(head.width);
    let badge = format!(" [{side}]");
    let first = Line::from(vec![
        Span::styled(
            fit(&title, width.saturating_sub(badge.len())),
            theme.strong(),
        ),
        Span::styled(
            badge,
            if view.version == Version::Before {
                theme.warn()
            } else {
                theme.good()
            },
        ),
    ]);
    let second = match &view.notice {
        Some(notice) => Line::from(Span::styled(fit(&clean(notice), width), theme.warn())),
        None => {
            let toggle = if view.other_version_exists {
                "v old/new · "
            } else {
                ""
            };
            Line::from(Span::styled(
                fit(
                    &format!("{toggle}[ ] changes · c comment · V select lines · Enter impact"),
                    width,
                ),
                theme.dim(),
            ))
        }
    };
    frame.render_widget(Paragraph::new(vec![first, second]), head);

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
    crate::actions::ensure_code_visible(app);

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
            let mark = mark_symbol(line.mark);
            let dot = if commented.contains(&line.number) {
                '●'
            } else {
                ' '
            };
            let mut spans = vec![Span::styled(
                format!("{mark}{:>number_w$} {dot}│", line.number),
                theme.mark(line.mark).add_modifier(if line.in_symbol {
                    Modifier::empty()
                } else {
                    Modifier::DIM
                }),
            )];
            spans.extend(code_spans(line, app.code.hscroll, text_w, theme));
            let mut out = Line::from(spans);
            if i == app.code.cursor {
                out = out.style(theme.selected());
            } else if selection.is_some_and(|(a, b)| (a..=b).contains(&i)) {
                out =
                    out.style(Style::default().add_modifier(Modifier::UNDERLINED | Modifier::BOLD));
            }
            out
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), text_area);

    if footer_h > 0 {
        let first = &at_cursor[0];
        let more = if at_cursor.len() > 1 {
            format!(" (+{} more)", at_cursor.len() - 1)
        } else {
            String::new()
        };
        let text = vec![
            Line::from(Span::styled(
                format!(
                    "● {}{more}",
                    fit(&first.0, usize::from(footer_area.width).saturating_sub(12))
                ),
                theme.warn(),
            )),
            Line::from(Span::styled(
                fit(&first.1, usize::from(footer_area.width)),
                theme.dim(),
            )),
        ];
        frame.render_widget(Paragraph::new(text), footer_area);
    }
}

/// Lines of the shown version that carry a comment, and the comments on the cursor line as
/// `(status and severity, text)`.
fn comment_lines(app: &App, view: &klaub_app::CodeView) -> (HashSet<u32>, Vec<(String, String)>) {
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
                format!("{severity}{}", labels::comment_status(comment.status)),
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
    let (head, body) = split_header(area);
    let subject = app.impact_subject().map_or_else(
        || "no symbol".to_owned(),
        |k| clean(&format!("{} ({})", k.qualified_name, k.file)),
    );
    header(frame, head, &format!("Impact · {subject}"), theme);
    let width = usize::from(body.width);
    let rows: Vec<Line<'static>> = app
        .impact
        .rows
        .iter()
        .map(|row| match row.kind {
            ImpactRowKind::Heading => {
                Line::from(Span::styled(fit(&row.text, width), theme.strong()))
            }
            ImpactRowKind::Note => Line::from(Span::styled(fit(&row.text, width), theme.dim())),
            ImpactRowKind::Entrypoint | ImpactRowKind::Caller | ImpactRowKind::Callee => {
                let mark = row.origin.map_or(' ', labels::origin_mark);
                let origin = row.origin.map_or("", labels::origin);
                let text = fit(&row.text, width.saturating_sub(origin.len() + 6));
                Line::from(vec![
                    Span::raw(format!("  {mark} {text}")),
                    Span::styled(format!("  {origin}"), theme.dim()),
                ])
            }
        })
        .collect();
    draw_list(frame, body, &mut app.impact_view, rows, theme);
}

// ---- comments -------------------------------------------------------------------------------------------------------

fn comments(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let total = app.session.review().comments.len();
    let (head, body) = split_header(area);
    header(
        frame,
        head,
        &format!("Comments · {total}  (Enter opens the code, e edit, d delete drafts)"),
        theme,
    );
    if total == 0 {
        empty(
            frame,
            body,
            "No comments yet. Open some code and press c.",
            theme,
        );
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
            let severity = c.severity.map_or("  ", |s| s.label());
            let status = labels::comment_status(c.status);
            let warn = if matches!(c.status, klaub_core::review::CommentStatus::Stale) {
                "⚠ "
            } else {
                ""
            };
            let head = format!(
                "{warn}{} {severity} {}{place} ",
                pad(status, 9),
                clean(c.anchor.file_path.as_str())
            );
            Line::from(vec![
                Span::styled(fit(&head, width / 2), theme.dim()),
                Span::raw(fit(&clean(&c.text), width.saturating_sub(width / 2 + 1))),
            ])
        })
        .collect();
    draw_list(frame, parts[0], &mut app.comments_view, rows, theme);
    if detail_h > 0
        && let Some(c) = app
            .session
            .review()
            .comments
            .get(app.comments_view.selected)
    {
        let mut lines = vec![Line::from(Span::styled(
            format!(
                "{} · anchor confidence {:.0}% · {}",
                labels::comment_status(c.status),
                c.anchor_confidence.value() * 100.0,
                c.anchor
                    .qualified_name
                    .as_deref()
                    .map_or_else(|| "file".to_owned(), clean),
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
        frame.render_widget(Paragraph::new(lines), parts[1]);
    }
}

/// Comment text keeps its line breaks but loses terminal sequences.
fn clean_multiline(text: &str) -> String {
    klaub_core::text::sanitize_terminal_text(text)
}

// ---- context ------------------------------------------------------------------------------------------------------------

fn context(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let (head, body) = split_header(area);
    header(
        frame,
        head,
        "Context · intent and acceptance criteria (you decide what is verified)",
        theme,
    );
    let brief = app.session.brief().clone();
    let width = usize::from(body.width);
    let mut rows: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled("Intent  ", theme.strong()),
        Span::raw(fit(
            &brief
                .intent_summary
                .as_deref()
                .map_or_else(|| "(not set — Enter to write it)".to_owned(), clean),
            width.saturating_sub(10),
        )),
    ])];
    for criterion in &brief.acceptance_criteria {
        rows.push(Line::from(vec![
            Span::styled(
                format!("{} ", labels::criterion_mark(criterion.status)),
                theme.good(),
            ),
            Span::raw(fit(&clean(&criterion.text), width.saturating_sub(18))),
            Span::styled(
                format!("  {}", labels::criterion_status(criterion.status)),
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
            "Text from a brief file is untrusted data. Enter changes status, a adds a criterion, i edits the intent.",
            theme.dim(),
        )));
        frame.render_widget(Paragraph::new(lines), parts[1]);
    }
}

// ---- tests -------------------------------------------------------------------------------------------------------------------

fn tests(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let (head, body) = split_header(area);
    let Some(unit) = app.unit.clone() else {
        header(frame, head, "Tests", theme);
        empty(frame, body, "Select a unit in the Queue first.", theme);
        return;
    };
    let label = app
        .current_item()
        .map_or_else(String::new, |i| clean(&i.unit.label));
    header(
        frame,
        head,
        &format!("Tests · {label}  (X runs them after asking, U reloads reports)"),
        theme,
    );
    let panel = app.session.test_panel(&unit);
    let width = usize::from(body.width).saturating_sub(2);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let status_style = match panel.status {
        EvidenceStatus::Passed => theme.good(),
        EvidenceStatus::Failed => theme.error(),
        EvidenceStatus::Stale | EvidenceStatus::NoAssociationFound => theme.warn(),
        EvidenceStatus::NotRun => theme.dim(),
    };
    lines.push(Line::from(vec![
        Span::raw("Evidence: "),
        Span::styled(panel.status.label().to_owned(), status_style),
    ]));
    let explanation = match panel.status {
        EvidenceStatus::NoAssociationFound => {
            "No test was linked to this symbol. That does not prove there is none: Klaub only links tests it can see calling or importing it."
        }
        EvidenceStatus::NotRun => {
            "Tests are associated, but no imported report has a result for them."
        }
        EvidenceStatus::Passed => {
            "Every associated test with a result passed in an imported report."
        }
        EvidenceStatus::Failed => "At least one associated test failed in an imported report.",
        EvidenceStatus::Stale => "The report is older than the newest change; do not trust it.",
    };
    for chunk in wrap(explanation, width) {
        lines.push(Line::from(Span::styled(chunk, theme.dim())));
    }
    if !panel.links.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled("Associated tests", theme.strong())));
        for link in &panel.links {
            lines.push(Line::from(format!(
                "  {} ({}:{}) via {} · {:.0}%",
                clean(&link.test.name),
                clean(link.test.file.as_str()),
                link.test.line,
                link.method.label(),
                link.confidence.value() * 100.0,
            )));
        }
    }
    if !panel.results.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled("Imported results", theme.strong())));
        for result in &panel.results {
            let style = if matches!(result.outcome, klaub_core::testing::TestOutcome::Failed) {
                theme.error()
            } else {
                theme.text()
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  {} — {}",
                    clean(&result.name),
                    labels::outcome(result.outcome)
                ),
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
        lines.push(Line::from(format!(
            "Coverage of {}: {} line(s) covered, {} missed (file level, from an imported report)",
            clean(coverage.file.as_str()),
            coverage.covered_lines.len(),
            coverage.missed_lines.len(),
        )));
    }
    lines.push(Line::raw(""));
    if panel.sources.is_empty() {
        lines.push(Line::from(Span::styled(
            "No test report is configured or found (tests.report_paths).",
            theme.dim(),
        )));
    }
    for source in &panel.sources {
        let stale = if source.stale { " · STALE" } else { "" };
        lines.push(Line::from(Span::styled(
            format!("Report: {}{stale}", clean(&source.path)),
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
        lines.push(Line::from(Span::styled(
            "Output of the last run (tail)",
            theme.strong(),
        )));
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
    let (head, body) = split_header(area);
    let comments: Vec<_> = app.followups().into_iter().cloned().collect();
    header(
        frame,
        head,
        &format!(
            "Follow-up · {} comment(s) from the previous round  (y resolved · p partial · u unresolved · X regression)",
            comments.len()
        ),
        theme,
    );
    if comments.is_empty() {
        empty(
            frame,
            body,
            "No comments were carried over. After you hand feedback to the agent and it changes the code, opening a new review continues from here.",
            theme,
        );
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
            let head = format!(
                "{} {} {}{place} ({confidence}) ",
                pad(c.followup_status.label(), 10),
                pad(labels::comment_status(c.status), 9),
                clean(c.anchor.file_path.as_str())
            );
            Line::from(vec![
                Span::styled(
                    fit(&head, width / 2),
                    if matches!(
                        c.followup_status,
                        klaub_core::review::FollowupStatus::Stale
                            | klaub_core::review::FollowupStatus::Regression
                    ) {
                        theme.warn()
                    } else {
                        theme.dim()
                    },
                ),
                Span::raw(fit(&clean(&c.text), width.saturating_sub(width / 2 + 1))),
            ])
        })
        .collect();
    draw_list(frame, body, &mut app.followup_view, rows, theme);
}

// ---- coverage ---------------------------------------------------------------------------------------------------------------------------

fn coverage(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let (head, body) = split_header(area);
    let cov = app.session.coverage();
    header(
        frame,
        head,
        "Coverage · what you marked as reviewed (never an approval)",
        theme,
    );
    let width = usize::from(body.width);
    let bar_w = (width / 4).clamp(6, 30);
    let mut top: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled("Overall  ", theme.strong()),
        Span::styled(bar(cov.reviewed, cov.total, bar_w), theme.good()),
        Span::raw(format!(
            " {}/{} ({}%)",
            cov.reviewed,
            cov.total,
            percent(cov.reviewed, cov.total)
        )),
    ])];
    for (priority, (done, all)) in &cov.by_priority {
        top.push(Line::from(vec![
            Span::styled(
                format!("  {}     ", priority.label()),
                theme.priority(*priority),
            ),
            Span::raw(format!("{} {done}/{all}", bar(*done, *all, bar_w))),
        ]));
    }
    if cov.unknown > 0 {
        top.push(Line::from(Span::styled(
            format!(
                "  {} unit(s) could not be analysed semantically; they are counted, not hidden.",
                cov.unknown
            ),
            theme.warn(),
        )));
    }
    if !cov.pending_critical.is_empty() {
        top.push(Line::from(Span::styled(
            format!(
                "  {} P0/P1 unit(s) not reviewed yet (N jumps to the next)",
                cov.pending_critical.len()
            ),
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
    frame.render_widget(Paragraph::new(top), parts[0]);
    let mut rows: Vec<Line<'static>> = cov
        .by_module
        .iter()
        .map(|(module, (done, all))| {
            Line::from(vec![
                Span::raw(pad(&clean(module), width.saturating_sub(bar_w + 14).max(8))),
                Span::styled(format!(" {}", bar(*done, *all, bar_w)), theme.good()),
                Span::raw(format!(" {done}/{all}")),
            ])
        })
        .collect();
    for note in &notes {
        rows.push(Line::from(Span::styled(
            format!("  notice: {}", fit(&clean(note), width.saturating_sub(12))),
            theme.dim(),
        )));
    }
    draw_list(frame, parts[1], &mut app.coverage_view, rows, theme);
}

// ---- flow ----------------------------------------------------------------------------------------------------------------------------------

fn flow(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let theme = app.theme;
    let (head, body) = split_header(area);
    let view = app.session.flow().clone();
    let width = usize::from(body.width).saturating_sub(2);
    let state_label = view.state.label();
    header(
        frame,
        head,
        &format!(
            "Flow · {state_label}  (Enter opens a step · ←/→/Space fold · p asks the agent · u reloads)"
        ),
        theme,
    );
    let mut lines: Vec<Line<'static>> = Vec::new();
    match (&view.state, &view.flow) {
        (FlowState::Missing, _) => {
            lines.push(Line::from(Span::styled(
                "Flow diagram not found",
                theme.warn(),
            )));
            lines.push(Line::from(Span::styled(
                format!("Expected at {}", clean(view.expected_path.as_str())),
                theme.dim(),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from("Press p to prepare a request for the agent (you confirm first, and Enter is never sent)."));
            lines.push(Line::from("The review works fully without a diagram."));
            lines.push(Line::raw(""));
            for chunk in wrap(&clean(&view.request_prompt), width) {
                lines.push(Line::from(Span::styled(chunk, theme.dim())));
            }
        }
        (_, None) => {
            lines.push(Line::from(Span::styled(
                "The flow file cannot be used",
                theme.error(),
            )));
            for issue in &view.issues {
                for chunk in wrap(&clean(issue), width) {
                    lines.push(Line::from(format!("  {chunk}")));
                }
            }
            lines.push(Line::from(Span::styled(
                "Fix the file or ask the agent again (p). It is never used partially.",
                theme.dim(),
            )));
        }
        (state, Some(validated)) => {
            if let FlowState::Stale { found } = state {
                lines.push(Line::from(Span::styled(
                    format!(
                        "Written for snapshot {} — not this one. Treat it as background only.",
                        clean(found)
                    ),
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
                format!("Written by {} — an explanation, not proof. ● resolved ◐ heuristic ○ unresolved", clean(&validated.document.generated_by)),
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
        frame.render_widget(Paragraph::new(lines), body);
        return;
    };
    let intro_h = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(body.height.saturating_sub(3));
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(intro_h), Constraint::Min(2)])
        .split(body);
    frame.render_widget(Paragraph::new(lines), parts[0]);
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
            fit_tail(&format!("path: {crumbs}"), list_width),
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
                _ => step
                    .external_ref
                    .as_ref()
                    .map_or_else(|| "external".to_owned(), |e| clean(&format!("{e:?}"))),
            };
            let missing = if resolved
                .is_some_and(|r| r.file.is_some() && !r.symbol_found && r.symbol.is_some())
            {
                " ⚠ symbol not found"
            } else {
                ""
            };
            let changed = if step.changed { " [changed]" } else { "" };
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
                    labels::edge_kind(kind),
                    labels::origin_mark(origin)
                )
            });
            let again = if line.row.revisit {
                " ↺ shown above"
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
