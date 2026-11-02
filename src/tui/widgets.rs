//! Small drawing helpers shared by the views.

use crate::domain::text::sanitize_single_line;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::theme::Theme;
use crate::tui::viewport::Viewport;

/// Text from the repository, a comment or a document, safe to draw on one line.
pub(crate) fn clean(text: &str) -> String {
    sanitize_single_line(text)
}

/// Cuts `text` to at most `width` terminal columns, ending with `…` when something was dropped.
pub(crate) fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// `text` cut or padded with spaces to exactly `width` columns.
pub(crate) fn pad(text: &str, width: usize) -> String {
    let fitted = fit(text, width);
    let missing = width.saturating_sub(fitted.width());
    format!("{fitted}{}", " ".repeat(missing))
}

/// A progress gauge `width` columns wide as `(filled, track)`. The filled part ends in an
/// eighth-block so progress reads smoothly, and it never claims "full" before `done == total`.
pub(crate) fn gauge(done: usize, total: usize, width: usize) -> (String, String) {
    const EIGHTHS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    if width == 0 {
        return (String::new(), String::new());
    }
    let cells = width * 8;
    let filled = match total {
        0 => 0,
        _ if done == 0 => 0,
        _ if done >= total => cells,
        _ => (done.saturating_mul(cells) / total).clamp(1, cells - 1),
    };
    let (whole, part) = (filled / 8, filled % 8);
    let mut head = "█".repeat(whole);
    let mut used = whole;
    if part > 0 {
        head.push(EIGHTHS[part]);
        used += 1;
    }
    (head, "░".repeat(width - used))
}

/// A percentage that never rounds a partial result up to "done".
pub(crate) fn percent(done: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let Some(raw) = done.checked_mul(100).and_then(|n| n.checked_div(total)) else {
        return 99;
    };
    raw.min(if done < total { 99 } else { 100 })
}

/// Like [`fit`], but keeps the end of the text and cuts the start, marking the cut with `…`. A
/// breadcrumb path is read from its tail, so that is the part that must survive a narrow terminal.
pub(crate) fn fit_tail(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 1; // the ellipsis
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        kept.push(c);
        used += w;
    }
    kept.reverse();
    std::iter::once('…').chain(kept).collect()
}

/// Splits `text` into lines no wider than `width` columns, breaking at spaces where possible.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split(' ') {
            let candidate_width = current.width() + usize::from(!current.is_empty()) + word.width();
            if !current.is_empty() && candidate_width > width {
                lines.push(std::mem::take(&mut current));
            }
            if word.width() > width {
                // A word longer than a line is cut.
                let mut chunk = String::new();
                for c in word.chars() {
                    if chunk.width() + c.width().unwrap_or(0) > width {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(c);
                }
                current = chunk;
                continue;
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        lines.push(current);
    }
    lines
}

/// A rounded panel with a title (and a quieter hint after it) on its top edge. Returns the area
/// inside the border. Lists draw their scrollbar on the right border, so they take this area as is.
pub(crate) fn panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    hint: &str,
    theme: Theme,
) -> Rect {
    panel_with(frame, area, title, hint, Vec::new(), theme)
}

/// Like [`panel`], with `badges` right-aligned on the top edge.
pub(crate) fn panel_with(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    hint: &str,
    badges: Vec<Span<'static>>,
    theme: Theme,
) -> Rect {
    let badge_w: usize = badges.iter().map(|s| s.content.width()).sum();
    let room = usize::from(area.width).saturating_sub(4 + badge_w + usize::from(badge_w > 0) * 2);
    let title = fit(title, room.saturating_sub(2));
    let used = title.width() + 2;
    let mut spans = vec![Span::styled(format!(" {title} "), theme.accent())];
    if !hint.is_empty() && room > used + 3 {
        spans.push(Span::styled(
            format!("{} ", fit(hint, room - used - 1)),
            theme.faint(),
        ));
    }
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title_top(Line::from(spans));
    if !badges.is_empty() {
        let mut right = vec![Span::raw(" ")];
        right.extend(badges);
        right.push(Span::raw(" "));
        block = block.title_top(Line::from(right).alignment(Alignment::Right));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// A rounded box for an overlay: the focus colour on the border, the title on the top edge.
pub(crate) fn dialog<'a>(title: &str, lines: Vec<Line<'a>>, theme: Theme) -> Paragraph<'a> {
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_focus())
            .title_top(Line::from(Span::styled(
                format!(" {title} "),
                theme.accent(),
            )))
            .padding(ratatui::widgets::Padding::horizontal(1)),
    )
}

/// `area` shrunk by `columns` on the left and right.
pub(crate) fn inset(area: Rect, columns: u16) -> Rect {
    let columns = columns.min(area.width / 2);
    Rect {
        x: area.x + columns,
        width: area.width - columns * 2,
        ..area
    }
}

/// A scrollbar on the border just right of `area`, drawn only when there is more than fits.
pub(crate) fn scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total: usize,
    offset: usize,
    theme: Theme,
) {
    let height = usize::from(area.height);
    let fits_frame = area.right() < frame.area().right();
    if total <= height || height == 0 || !fits_frame {
        return;
    }
    let mut state = ScrollbarState::new(total - height)
        .viewport_content_length(height)
        .position(offset.min(total - height));
    let track = Rect {
        width: area.width + 1,
        ..area
    };
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("┃")
            .track_style(theme.border())
            .thumb_style(theme.accent()),
        track,
        &mut state,
    );
}

/// Centred, quiet text for a view that has nothing to show.
pub(crate) fn empty_state(frame: &mut Frame<'_>, area: Rect, text: &str, theme: Theme) {
    let width = usize::from(area.width).saturating_sub(6).clamp(1, 64);
    let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled("◇", theme.faint()))];
    lines.extend(
        wrap(text, width)
            .into_iter()
            .map(|l| Line::from(Span::styled(l, theme.dim()))),
    );
    let top = area.y.saturating_add(
        area.height
            .saturating_sub(u16::try_from(lines.len()).unwrap_or(0))
            / 2,
    );
    let place = Rect {
        y: top,
        height: area.height.saturating_sub(top - area.y),
        ..area
    };
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), place);
}

/// Draws the visible window of a list, highlighting the selected row, and records the height.
pub(crate) fn draw_list(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &mut Viewport,
    rows: Vec<Line<'static>>,
    theme: Theme,
) {
    view.height = usize::from(area.height);
    view.area = area;
    view.len = rows.len();
    view.clamp(rows.len());
    let window = view.window(rows.len());
    let selected = view.selected;
    let total = rows.len();
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .enumerate()
        .skip(window.start)
        .take(window.len())
        .map(|(i, line)| {
            if i == selected {
                let bar = if theme.is_high_contrast() {
                    "▶"
                } else {
                    "▌"
                };
                let mut spans = vec![Span::styled(bar, theme.accent())];
                spans.extend(line.spans);
                fill_line(
                    Line::from(spans).style(theme.selected()),
                    usize::from(area.width),
                )
            } else {
                let mut spans = vec![Span::raw(" ")];
                spans.extend(line.spans);
                Line::from(spans)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
    scrollbar(frame, area, total, view.offset, theme);
}

/// `line` extended with spaces to `width` columns, so a background set on the line reaches the
/// right edge (a paragraph only paints the cells its text covers).
pub(crate) fn fill_line(mut line: Line<'static>, width: usize) -> Line<'static> {
    let missing = width.saturating_sub(line.width());
    if missing > 0 {
        line.spans.push(Span::raw(" ".repeat(missing)));
    }
    line
}

/// A solid label: a space of padding on each side of `text`.
pub(crate) fn chip_span(text: &str, style: Style) -> Span<'static> {
    Span::styled(format!(" {text} "), style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_counts_columns_not_bytes() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello…");
        assert_eq!(fit("日本語のテキスト", 7), "日本語…");
        assert_eq!(fit("anything", 0), "");
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(pad("日本", 5).width(), 5);
    }

    #[test]
    fn gauges_never_claim_done_early() {
        let columns = |g: (String, String)| g.0.width() + g.1.width();
        assert_eq!(gauge(0, 10, 4), (String::new(), "░░░░".to_owned()));
        assert_eq!(gauge(10, 10, 4), ("████".to_owned(), String::new()));
        assert_eq!(gauge(3, 0, 4).0, "");
        assert_ne!(gauge(99, 100, 4).0, "████", "99% is not full");
        assert!(!gauge(1, 1000, 4).0.is_empty(), "some progress shows");
        for done in 0..=20 {
            assert_eq!(columns(gauge(done, 20, 7)), 7);
        }
        assert_eq!(percent(99, 100), 99);
        assert_eq!(percent(999, 1000), 99);
        assert_eq!(percent(1000, 1000), 100);
        assert_eq!(percent(0, 0), 0);
    }

    #[test]
    fn wrapping_respects_width_and_long_words() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\nb", 10), vec!["a", "b"]);
        assert_eq!(wrap("", 5).len(), 1);
    }
}
