//! Text helpers: terminal sanitisation, truncation and line indexing.
//!
//! Source code, file names and review comments are untrusted. Before any of
//! them is drawn on the screen or pasted into another program's input it must
//! pass through [`sanitize_terminal_text`], so that it cannot move the cursor,
//! rewrite the window title, change the clipboard (OSC 52) or hide content with
//! bidirectional overrides.

/// Removes everything from `input` that could act on a terminal.
///
/// Removed: ANSI escape sequences (CSI, OSC, DCS, APC, PM, SOS and simple
/// two-byte escapes), C0 control characters other than `\n` and `\t`, `\r`,
/// DEL, C1 control characters (U+0080–U+009F) and bidirectional overrides and
/// isolates (U+202A–U+202E, U+2066–U+2069).
pub fn sanitize_terminal_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => skip_escape_sequence(&mut chars),
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Like [`sanitize_terminal_text`] but also collapses every line break into a space.
pub fn sanitize_single_line(input: &str) -> String {
    sanitize_terminal_text(input)
        .split(['\n', '\t'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn skip_escape_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.next() {
        // CSI: parameters/intermediates until a final byte in 0x40..=0x7E.
        Some('[') => {
            for c in chars.by_ref() {
                if ('\u{40}'..='\u{7E}').contains(&c) {
                    break;
                }
            }
        }
        // OSC, DCS, SOS, PM, APC: string terminated by BEL or ST (ESC \).
        Some(']' | 'P' | 'X' | '^' | '_') => {
            while let Some(c) = chars.next() {
                if c == '\x07' {
                    break;
                }
                if c == '\x1b' {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
            }
        }
        // Escape with intermediate bytes (0x20..=0x2F) then a final byte, e.g. ESC ( B.
        Some(c) if ('\u{20}'..='\u{2F}').contains(&c) => {
            while let Some(&next) = chars.peek() {
                chars.next();
                if ('\u{30}'..='\u{7E}').contains(&next) {
                    break;
                }
            }
        }
        // Two-byte escape such as ESC c: the second byte was consumed above.
        Some(_) | None => {}
    }
}

/// Shortens `input` to at most `max_chars` characters, ending with `…` when cut.
pub fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out: String = input.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

/// Collapses runs of whitespace into single spaces and trims the ends.
pub fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Maps byte offsets to 1-based line numbers.
#[derive(Debug, Clone)]
pub struct LineIndex {
    starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    /// Indexes `text`.
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self {
            starts,
            len: text.len(),
        }
    }

    /// Number of lines (a trailing newline does not start a new line).
    pub fn line_count(&self) -> u32 {
        let trailing_newline = self.starts.last() == Some(&self.len) && self.starts.len() > 1;
        let lines = self.starts.len() - usize::from(trailing_newline);
        u32::try_from(lines).unwrap_or(u32::MAX)
    }

    /// The 1-based line containing `offset`.
    pub fn line_of(&self, offset: usize) -> u32 {
        let line = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        u32::try_from(line + 1).unwrap_or(u32::MAX)
    }

    /// Byte offset of the start of a 1-based line, if it exists.
    pub fn line_start(&self, line: u32) -> Option<usize> {
        let idx = usize::try_from(line).ok()?.checked_sub(1)?;
        self.starts.get(idx).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn strips_csi_osc_and_controls() {
        assert_eq!(sanitize_terminal_text("a\x1b[31mred\x1b[0m b"), "ared b");
        assert_eq!(sanitize_terminal_text("x\x1b]0;evil title\x07y"), "xy");
        assert_eq!(sanitize_terminal_text("x\x1b]52;c;ZXZpbA==\x1b\\y"), "xy");
        assert_eq!(sanitize_terminal_text("a\rb\x08c\x7fd"), "abcd");
        assert_eq!(sanitize_terminal_text("a\u{9b}31mb"), "a31mb");
        assert_eq!(sanitize_terminal_text("ok\u{202E}gpj.exe"), "okgpj.exe");
    }

    #[test]
    fn keeps_newlines_tabs_and_unicode() {
        assert_eq!(
            sanitize_terminal_text("línea\n\tçã 漢字 🦀"),
            "línea\n\tçã 漢字 🦀"
        );
    }

    #[test]
    fn handles_truncated_and_odd_escapes() {
        assert_eq!(sanitize_terminal_text("a\x1b"), "a");
        assert_eq!(sanitize_terminal_text("a\x1b[31"), "a");
        assert_eq!(sanitize_terminal_text("a\x1bcb"), "ab");
        assert_eq!(sanitize_terminal_text("a\x1b(Bb"), "ab");
        assert_eq!(sanitize_terminal_text("a\x1bPdcs data\x1b\\b"), "ab");
    }

    #[test]
    fn bracketed_paste_end_marker_is_neutralised() {
        let hostile = "text\x1b[201~; rm -rf /";
        assert!(!sanitize_terminal_text(hostile).contains('\x1b'));
    }

    #[test]
    fn single_line_collapses_breaks() {
        assert_eq!(sanitize_single_line("a\n  b\t c\r\n"), "a b c");
    }

    #[test]
    fn truncation_counts_characters() {
        assert_eq!(truncate_chars("héllo wörld", 5), "héll…");
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abc", 0), "");
    }

    #[test]
    fn line_index_maps_offsets() {
        let text = "one\ntwo\nthree\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_of(0), 1);
        assert_eq!(index.line_of(3), 1);
        assert_eq!(index.line_of(4), 2);
        assert_eq!(index.line_of(text.len() - 1), 3);
        assert_eq!(index.line_start(2), Some(4));
        assert_eq!(index.line_start(0), None);
        assert_eq!(LineIndex::new("").line_count(), 1);
        assert_eq!(LineIndex::new("a").line_count(), 1);
    }

    proptest! {
        #[test]
        fn sanitised_text_never_contains_escape_or_controls(input in any::<String>()) {
            let clean = sanitize_terminal_text(&input);
            prop_assert!(!clean.chars().any(|c| c.is_control() && c != '\n' && c != '\t'));
        }

        #[test]
        fn sanitising_is_idempotent(input in any::<String>()) {
            let once = sanitize_terminal_text(&input);
            prop_assert_eq!(sanitize_terminal_text(&once), once);
        }
    }
}
