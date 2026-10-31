//! Line-level diff for the reader: which lines changed on each side.

use std::time::Duration;

use klaub_core::semantic::{Hunk, LineDiff, LineMark, LineRange};
use similar::{DiffTag, TextDiff};

/// Longest time spent diffing one file before falling back to a coarser result.
const TIMEOUT: Duration = Duration::from_millis(750);

fn range(start: usize, len: usize) -> Option<LineRange> {
    (len > 0).then(|| LineRange {
        start: u32::try_from(start + 1).unwrap_or(u32::MAX),
        end: u32::try_from(start + len).unwrap_or(u32::MAX),
    })
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

/// Computes per-line marks for both sides and the contiguous changed regions.
///
/// A missing side counts as empty, so a new file is entirely `Added` and a deleted file
/// entirely `Removed`.
pub fn line_diff(before: Option<&str>, after: Option<&str>) -> LineDiff {
    let (before, after) = (before.unwrap_or(""), after.unwrap_or(""));
    let mut result = LineDiff {
        before: vec![LineMark::Same; line_count(before)],
        after: vec![LineMark::Same; line_count(after)],
        hunks: Vec::new(),
    };
    let diff = TextDiff::configure()
        .timeout(TIMEOUT)
        .diff_lines(before, after);
    for op in diff.ops() {
        let (old, new) = (op.old_range(), op.new_range());
        let (mark_before, mark_after) = match op.tag() {
            DiffTag::Equal => continue,
            DiffTag::Delete => (LineMark::Removed, LineMark::Same),
            DiffTag::Insert => (LineMark::Same, LineMark::Added),
            DiffTag::Replace => (LineMark::Changed, LineMark::Changed),
        };
        for line in old.clone() {
            if let Some(mark) = result.before.get_mut(line) {
                *mark = mark_before;
            }
        }
        for line in new.clone() {
            if let Some(mark) = result.after.get_mut(line) {
                *mark = mark_after;
            }
        }
        result.hunks.push(Hunk {
            before: range(old.start, old.len()),
            after: range(new.start, new.len()),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_replaced_added_and_removed_lines() {
        let before = "a\nb\nc\nd\n";
        let after = "a\nB\nc\nd\ne\n";
        let diff = line_diff(Some(before), Some(after));
        assert_eq!(
            diff.before,
            vec![
                LineMark::Same,
                LineMark::Changed,
                LineMark::Same,
                LineMark::Same
            ]
        );
        assert_eq!(
            diff.after,
            vec![
                LineMark::Same,
                LineMark::Changed,
                LineMark::Same,
                LineMark::Same,
                LineMark::Added
            ]
        );
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!(diff.hunks[0].after, Some(LineRange { start: 2, end: 2 }));
        assert_eq!(
            diff.hunks[1].before, None,
            "a pure insertion has no before range"
        );
        assert_eq!(diff.hunks[1].after, Some(LineRange { start: 5, end: 5 }));
    }

    #[test]
    fn deletions_mark_only_the_before_side() {
        let diff = line_diff(Some("a\nb\nc\n"), Some("a\nc\n"));
        assert_eq!(
            diff.before,
            vec![LineMark::Same, LineMark::Removed, LineMark::Same]
        );
        assert_eq!(diff.after, vec![LineMark::Same, LineMark::Same]);
        assert_eq!(diff.hunks[0].after, None);
    }

    #[test]
    fn new_and_deleted_files() {
        let new = line_diff(None, Some("x\ny\n"));
        assert!(new.before.is_empty());
        assert_eq!(new.after, vec![LineMark::Added, LineMark::Added]);
        let gone = line_diff(Some("x\ny\n"), None);
        assert_eq!(gone.before, vec![LineMark::Removed, LineMark::Removed]);
        assert!(gone.after.is_empty());
    }

    #[test]
    fn identical_text_has_no_hunks() {
        let diff = line_diff(Some("a\nb\n"), Some("a\nb\n"));
        assert!(diff.hunks.is_empty());
        assert!(diff.after.iter().all(|m| *m == LineMark::Same));
    }

    #[test]
    fn a_missing_trailing_newline_is_a_change() {
        let diff = line_diff(Some("a\nb"), Some("a\nb\n"));
        assert!(!diff.hunks.is_empty());
    }
}
