//! Small drawing helpers shared by the views.

use klaub_core::text::sanitize_single_line;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Theme;
use crate::viewport::Viewport;

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

/// A text progress bar such as `████░░░░`.
pub(crate) fn bar(done: usize, total: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = match total {
        0 => 0,
        _ if done == 0 => 0,
        _ => (done * width).div_ceil(total).clamp(1, width),
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
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

/// Draws the visible window of a list, highlighting the selected row, and records the height.
pub(crate) fn draw_list(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &mut Viewport,
    rows: Vec<Line<'static>>,
    theme: Theme,
) {
    view.height = usize::from(area.height);
    view.clamp(rows.len());
    let window = view.window(rows.len());
    let selected = view.selected;
    let lines: Vec<Line<'static>> =
        rows.into_iter()
            .enumerate()
            .skip(window.start)
            .take(window.len())
            .map(|(i, line)| {
                if i == selected {
                    let marked =
                        Line::from(
                            std::iter::once(Span::raw("▶"))
                                .chain(line.spans.into_iter().map(|s| {
                                    Span::styled(s.content, s.style.patch(theme.selected()))
                                }))
                                .collect::<Vec<_>>(),
                        );
                    marked.style(theme.selected())
                } else {
                    Line::from(
                        std::iter::once(Span::raw(" "))
                            .chain(line.spans)
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();
    frame.render_widget(Paragraph::new(lines), area);
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
    fn bars_never_claim_done_early() {
        assert_eq!(bar(0, 10, 4), "░░░░");
        assert_eq!(bar(1, 100, 4), "█░░░");
        assert_eq!(bar(10, 10, 4), "████");
        assert_eq!(bar(3, 0, 4), "░░░░");
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
