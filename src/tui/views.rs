//! The main area for each mode. Everything that comes from the repository, a comment or a document
//! goes through [`clean`] before it reaches the screen.

use crate::app::CodeLine;
use crate::domain::config::Density;
use crate::domain::finding::QueueItem;
use crate::domain::i18n::tui as t;
use crate::domain::review::Version;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::tui::app::{App, Mode, TreeKind};
use crate::tui::labels;
use crate::tui::theme::{Chip, Theme, mark_symbol};
use crate::tui::widgets::{
    chip_span, clean, draw_cards, empty_state, fill_line, fit, fit_tail, pad, panel, panel_with,
    scrollbar, vline, wrap,
};

/// Draws the main area for the current mode.
pub(crate) fn draw_mode(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    workspace(frame, area, app);
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
    // The check and the priority, then two spaces: the second line starts under the name.
    let lead = 6;
    let label_w = width.saturating_sub(lead);
    let label_style = if reviewed {
        theme.dim()
    } else {
        theme.strong()
    };
    // Every file has a `<module>`, so on its own the name says nothing: add the file it is in.
    let label = clean(&item.unit.label);
    let file_name = clean(item.unit.file.as_str());
    let file_name = file_name
        .rsplit('/')
        .next()
        .unwrap_or(&file_name)
        .to_owned();
    let mut name = vec![Span::styled(label.clone(), label_style)];
    let mut used = label.width();
    if item.unit.symbol_kind == Some(crate::domain::symbol::SymbolKind::Module) {
        let tail = fit(&file_name, label_w.saturating_sub(used + 2));
        used += 2 + tail.width();
        name.push(Span::styled(format!("  {tail}"), theme.dim()));
    }
    let name_pad = label_w.saturating_sub(used);
    let mut top_spans = vec![
        Span::styled(if reviewed { "✓ " } else { "  " }, theme.good()),
        Span::styled(item.priority.label(), theme.priority(item.priority)),
        Span::raw("  "),
    ];
    top_spans.extend(name);
    top_spans.push(Span::raw(" ".repeat(name_pad)));
    let top = Line::from(top_spans);

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

    let text_area = body;
    app.code.height = usize::from(text_area.height);
    app.code.area = text_area;
    crate::tui::actions::ensure_code_visible(app);

    let number_w = view
        .lines
        .last()
        .map_or(3, |l| l.number.to_string().len())
        .max(3);
    let gutter_w = number_w + 3;
    let text_w = usize::from(text_area.width).saturating_sub(gutter_w);
    let lines: Vec<Line<'static>> = view
        .lines
        .iter()
        .enumerate()
        .skip(app.code.offset)
        .take(usize::from(text_area.height))
        .map(|(i, line)| {
            let is_cursor = i == app.code.cursor && app.mode == Mode::Code;
            let mark = mark_symbol(line.mark);
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
            }
            out
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), text_area);
    scrollbar(frame, text_area, view.lines.len(), app.code.offset, theme);
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
pub(crate) fn section(title: &str, _width: usize, theme: Theme) -> Vec<Line<'static>> {
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

    // The signature, with its types. A file's top-level code has none, so it says what it is.
    if item.is_some_and(|i| i.unit.symbol_kind == Some(crate::domain::symbol::SymbolKind::Module)) {
        lines.push(Line::from(""));
        for chunk in wrap(t::inspector_module_note(l), width) {
            lines.push(Line::from(Span::styled(chunk, theme.dim())));
        }
    } else if let Some(d) = &delta {
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
            lines.extend(delta_lines(
                change,
                d.before_signature.is_some(),
                width,
                theme,
            ));
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
pub(crate) fn signature_lines(
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
            (Some(text), _) if !text.trim().is_empty() => wrap(&clean(text), width)
                .into_iter()
                .map(|chunk| Line::from(Span::styled(chunk, theme.text())))
                .collect(),
            (None, Some(old)) if !old.trim().is_empty() => wrap(&clean(old), width)
                .into_iter()
                .map(|chunk| Line::from(Span::styled(chunk, theme.faint())))
                .collect(),
            _ => Vec::new(),
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
pub(crate) fn delta_lines(
    change: &crate::app::Delta,
    existed_before: bool,
    width: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for item in &change.after {
        // Everything in a new symbol is new, so only a symbol that existed has entries to mark.
        let added = existed_before && !change.before.contains(item);
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
