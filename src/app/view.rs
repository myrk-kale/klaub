//! What the reader shows: code with change marks and syntax colours.

use crate::domain::graph::SymbolKey;
use crate::domain::ids::UnitId;
use crate::domain::language::Language;
use crate::domain::path::RepoPath;
use crate::domain::ports::SourceParser;
use crate::domain::review::Version;
use crate::domain::semantic::{ChangeKind, LineMark};
use crate::domain::snapshot::ChangeStatus;
use crate::domain::symbol::HighlightClass;
use crate::domain::text::{LineIndex, sanitize_terminal_text};

use crate::app::Analysis;

/// How much code to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeScope {
    /// Only the selected symbol.
    Symbol,
    /// The whole file.
    File,
}

/// A coloured range inside one displayed line, in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightedSpan {
    /// First character (inclusive).
    pub start: usize,
    /// Last character (exclusive).
    pub end: usize,
    /// The colour class.
    pub class: HighlightClass,
}

/// One line of code ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLine {
    /// 1-based line number in the file version being shown.
    pub number: u32,
    /// The text, with tabs expanded and terminal sequences removed.
    pub text: String,
    /// Whether this line changed relative to the other version.
    pub mark: LineMark,
    /// Syntax colours.
    pub spans: Vec<HighlightedSpan>,
    /// The line belongs to the selected symbol (all lines do in symbol scope).
    pub in_symbol: bool,
}

/// A rendering-ready view of one version of a unit's code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeView {
    /// The unit shown; `None` for a project symbol that is not part of the change.
    pub unit: Option<UnitId>,
    /// Its file.
    pub path: RepoPath,
    /// Which version.
    pub version: Version,
    /// Symbol or file scope.
    pub scope: CodeScope,
    /// The lines.
    pub lines: Vec<CodeLine>,
    /// Why there is nothing (or something unusual) to show: a new file, a removed symbol, binary data.
    pub notice: Option<String>,
    /// First lines of change regions in this version, for `[` and `]`.
    pub changes: Vec<u32>,
    /// The symbol's line range in this version.
    pub symbol_lines: Option<(u32, u32)>,
    /// The other version exists, so `v` has something to toggle to.
    pub other_version_exists: bool,
}

const TAB_WIDTH: usize = 4;

/// The largest char boundary of `text` that is not after `index` (`str::floor_char_boundary` needs Rust 1.91).
fn char_boundary_at_or_before(text: &str, index: usize) -> usize {
    (0..=index.min(text.len()))
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0)
}

/// Expands tabs, drops terminal sequences, and carries highlight ranges (in raw characters) along.
fn display(raw: &str, ranges: &[(usize, usize, HighlightClass)]) -> (String, Vec<HighlightedSpan>) {
    let clean = sanitize_terminal_text(raw);
    let mut text = String::with_capacity(clean.len());
    let mut map: Vec<usize> = Vec::with_capacity(clean.chars().count() + 1);
    for c in clean.chars() {
        map.push(text.chars().count());
        if c == '\t' {
            text.extend(std::iter::repeat_n(' ', TAB_WIDTH));
        } else {
            text.push(c);
        }
    }
    map.push(text.chars().count());
    let spans = ranges
        .iter()
        .filter_map(|(start, end, class)| {
            let (s, e) = (*map.get(*start)?, *map.get((*end).min(map.len() - 1))?);
            (e > s).then_some(HighlightedSpan {
                start: s,
                end: e,
                class: *class,
            })
        })
        .collect();
    (text, spans)
}

impl Analysis {
    /// Builds the view of `unit` in `version`.
    ///
    /// A missing counterpart is explained, never invented: a new file has no "before", and the
    /// reader says so instead of showing an empty comparison.
    pub fn code_view(
        &self,
        parser: &dyn SourceParser,
        unit: &UnitId,
        version: Version,
        scope: CodeScope,
    ) -> Option<CodeView> {
        let info = self.units.get(unit)?;
        let owner = &self.files[info.file_index];
        let symbol_change = info.change_index.map(|i| &owner.analysis.changes[i]);
        let other = match version {
            Version::After => Version::Before,
            Version::Before => Version::After,
        };
        // A symbol that moved between files is filed under the file it moved to, but its old code
        // is in the file it moved from. Each version is read from the file it belongs to.
        let file_at = |v: Version| match (v, symbol_change.and_then(|c| c.before.as_ref())) {
            (Version::Before, Some(side)) => {
                crate::app::analysis::index_for(&self.files, side).unwrap_or(info.file_index)
            }
            _ => info.file_index,
        };
        let entry = &self.files[file_at(version)];
        let change = &self.snapshot.files[file_at(version)];
        let text_of = |v: Version| match v {
            Version::After => self.snapshot.files[file_at(v)].after_text(),
            Version::Before => self.snapshot.files[file_at(v)].before_text(),
        };
        let exists = |v: Version| match (v, symbol_change) {
            (Version::After, Some(c)) => c.after.is_some(),
            (Version::Before, Some(c)) => c.before.is_some(),
            (v, None) => text_of(v).is_some(),
        };
        let mut view = CodeView {
            unit: Some(unit.clone()),
            path: entry.analysis.path.clone(),
            version,
            scope,
            lines: Vec::new(),
            notice: None,
            changes: Vec::new(),
            symbol_lines: None,
            other_version_exists: exists(other),
        };

        let Some(text) = text_of(version) else {
            view.notice = Some(match (version, change.status) {
                (Version::Before, ChangeStatus::Added) => "NEW · no previous version".to_owned(),
                (Version::After, ChangeStatus::Deleted) => {
                    "REMOVED · the file no longer exists in the new version".to_owned()
                }
                _ => match &entry.analysis.support {
                    crate::domain::semantic::SemanticSupport::TextOnly(why) => {
                        format!("not shown as text: {why}")
                    }
                    crate::domain::semantic::SemanticSupport::Full => {
                        "this version has no readable content".to_owned()
                    }
                },
            });
            return Some(view);
        };
        if let Some(c) = symbol_change {
            let side = match version {
                Version::After => c.after.as_ref(),
                Version::Before => c.before.as_ref(),
            };
            match side {
                Some(r) => view.symbol_lines = Some((r.span.start_line, r.span.end_line)),
                None => {
                    view.notice = Some(match (version, c.kind) {
                        (Version::Before, ChangeKind::Added) => {
                            "NEW · no previous version of this symbol, nothing to compare"
                                .to_owned()
                        }
                        (Version::After, ChangeKind::Removed) => {
                            "REMOVED · this symbol does not exist in the new version".to_owned()
                        }
                        _ => "this version of the symbol does not exist".to_owned(),
                    });
                    if scope == CodeScope::Symbol {
                        return Some(view);
                    }
                }
            }
        }

        let range = match (scope, view.symbol_lines) {
            (CodeScope::Symbol, Some((start, end))) => start as usize..=end as usize,
            _ => 1..=text.lines().count(),
        };
        let marks = match version {
            Version::After => &entry.analysis.line_diff.after,
            Version::Before => &entry.analysis.line_diff.before,
        };
        view.lines = render_lines(
            parser,
            entry.analysis.language,
            text,
            marks,
            range,
            view.symbol_lines,
        );

        // Starts of change regions inside what is shown.
        let first_shown = view.lines.first().map_or(0, |l| l.number);
        let last_shown = view.lines.last().map_or(0, |l| l.number);
        for hunk in &entry.analysis.line_diff.hunks {
            let region = match version {
                Version::After => hunk.after,
                Version::Before => hunk.before,
            };
            // A pure deletion has no lines in the new version: point at the line after the gap.
            let start = region
                .map(|r| r.start)
                .or_else(|| match (version, hunk.before) {
                    (Version::After, Some(b)) => Some(b.start.min(last_shown.max(1))),
                    _ => None,
                });
            if let Some(start) = start.filter(|s| (first_shown..=last_shown).contains(s)) {
                view.changes.push(start);
            }
        }
        view.changes.sort_unstable();
        view.changes.dedup();
        Some(view)
    }
}

impl Analysis {
    /// The code of a project symbol that is not part of the change (a caller reached through the
    /// impact view), read from the "after" side. `text` is the file's content.
    pub fn symbol_view(
        &self,
        parser: &dyn SourceParser,
        key: &SymbolKey,
        text: &str,
    ) -> Option<CodeView> {
        let parsed = self.parsed.get(&key.file)?;
        let symbol = &parsed.symbols[parsed.find(&key.qualified_name)?];
        let marks: &[LineMark] = self
            .file(&key.file)
            .map_or(&[], |entry| entry.analysis.line_diff.after.as_slice());
        let symbol_lines = Some((symbol.span.start_line, symbol.span.end_line));
        let lines = render_lines(
            parser,
            Some(parsed.language),
            text,
            marks,
            symbol.span.start_line as usize..=symbol.span.end_line as usize,
            symbol_lines,
        );
        let changes = lines
            .iter()
            .filter(|l| l.mark != LineMark::Same)
            .map(|l| l.number)
            .collect();
        Some(CodeView {
            unit: None,
            path: key.file.clone(),
            version: Version::After,
            scope: CodeScope::Symbol,
            lines,
            notice: Some("not part of this change · shown for context".to_owned()),
            changes,
            symbol_lines,
            other_version_exists: false,
        })
    }
}

/// Turns source text into drawable lines: highlighting, tab expansion, sanitising and change marks.
fn render_lines(
    parser: &dyn SourceParser,
    language: Option<Language>,
    text: &str,
    marks: &[LineMark],
    range: std::ops::RangeInclusive<usize>,
    symbol_lines: Option<(u32, u32)>,
) -> Vec<CodeLine> {
    let spans = language
        .and_then(|l| parser.highlight(l, text).ok())
        .unwrap_or_default();
    let index = LineIndex::new(text);
    let raw_lines: Vec<&str> = text.lines().collect();
    let mut per_line: Vec<Vec<(usize, usize, HighlightClass)>> = vec![Vec::new(); raw_lines.len()];
    for span in &spans {
        let first = index.line_of(span.start) as usize;
        let last = index.line_of(span.end.saturating_sub(1).max(span.start)) as usize;
        for line in first..=last.min(raw_lines.len()) {
            let Some(start_byte) = index.line_start(u32::try_from(line).unwrap_or(u32::MAX)) else {
                continue;
            };
            let raw = raw_lines[line - 1];
            let from = if line == first {
                text[start_byte..]
                    .char_indices()
                    .find(|(i, _)| start_byte + i >= span.start)
                    .map_or(0, |(i, _)| text[start_byte..start_byte + i].chars().count())
            } else {
                0
            };
            let to = if line == last {
                let end_in_line = span.end.saturating_sub(start_byte).min(raw.len());
                raw[..char_boundary_at_or_before(raw, end_in_line)]
                    .chars()
                    .count()
            } else {
                raw.chars().count()
            };
            if to > from {
                per_line[line - 1].push((from, to, span.class));
            }
        }
    }

    let mut lines = Vec::new();
    for (i, raw) in raw_lines.iter().enumerate() {
        let number = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if !range.contains(&(i + 1)) {
            continue;
        }
        let (text, spans) = display(raw, &per_line[i]);
        let in_symbol = symbol_lines.is_none_or(|(s, e)| (s..=e).contains(&number));
        lines.push(CodeLine {
            number,
            text,
            mark: marks.get(i).copied().unwrap_or(LineMark::Same),
            spans,
            in_symbol,
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_and_spans_follow() {
        let (text, spans) = display(
            "\tdef f(): # note",
            &[
                (1, 4, HighlightClass::Keyword),
                (10, 16, HighlightClass::Comment),
            ],
        );
        assert_eq!(text, "    def f(): # note");
        assert_eq!(
            spans[0],
            HighlightedSpan {
                start: 4,
                end: 7,
                class: HighlightClass::Keyword
            }
        );
        assert_eq!(
            spans[1],
            HighlightedSpan {
                start: 13,
                end: 19,
                class: HighlightClass::Comment
            }
        );
    }

    #[test]
    fn terminal_sequences_never_reach_the_view() {
        let (text, spans) = display("a\x1b[31mb\x07c", &[(0, 3, HighlightClass::String)]);
        assert_eq!(text, "abc");
        assert!(!text.contains('\x1b'));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].end, 3);
    }
}
