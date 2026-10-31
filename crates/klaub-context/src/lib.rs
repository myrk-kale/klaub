//! # klaub-context
//!
//! A pure domain service for what the *human* knows and has done:
//!
//! - the **brief**: an intent summary and acceptance criteria written by the reviewer (or imported
//!   from a file they name). Klaub never infers intent from an agent's history or a tracker;
//! - **coverage** of the human review: which units the reviewer explicitly marked as reviewed,
//!   what is still pending, and a non-blocking warning before finishing. Opening a file, running
//!   tests or having no alerts never counts as reviewed.

use std::collections::{BTreeMap, HashSet};

use klaub_core::config::CountUnit;
use klaub_core::finding::Priority;
use klaub_core::ids::{CriterionId, UnitId};
use klaub_core::path::RepoPath;
use klaub_core::review::{Brief, Criterion, CriterionStatus};
use klaub_core::text::{sanitize_single_line, truncate_chars};
use klaub_core::{Error, Result};

/// Longest accepted brief file.
pub const MAX_BRIEF_BYTES: usize = 64 * 1024;
const MAX_CRITERIA: usize = 100;
const MAX_CRITERION_CHARS: usize = 500;

/// Parses a Markdown brief given with `klaub review --brief <file>`.
///
/// - the first `#` heading, or the first paragraph under it, or a section named *Intent*,
///   *Objective*, *Goal* or *Summary*, is the intent summary;
/// - a line `Task: ...`, `Ticket: ...` or `Issue: ...` is the task reference;
/// - every task-list item (`- [ ] text`, `- [x] text`) is an acceptance criterion. Imported criteria
///   start as `not_checked` **even when the file ticks them**: verifying is the reviewer's job.
pub fn parse_brief(markdown: &str) -> Result<Brief> {
    if markdown.len() > MAX_BRIEF_BYTES {
        return Err(Error::invalid(
            "brief",
            format!("larger than {MAX_BRIEF_BYTES} bytes"),
        ));
    }
    let mut intent: Option<String> = None;
    let mut task_ref: Option<String> = None;
    let mut criteria: Vec<Criterion> = Vec::new();
    let mut section = String::new();
    let mut paragraph = String::new();
    let mut first_heading: Option<String> = None;

    let flush = |paragraph: &mut String, section: &str, intent: &mut Option<String>| {
        let text = sanitize_single_line(paragraph);
        paragraph.clear();
        if text.is_empty() {
            return;
        }
        let wanted = matches!(
            section,
            "intent" | "objective" | "goal" | "summary" | "context"
        );
        if intent.is_none() && (wanted || section.is_empty() || section == "__first__") {
            *intent = Some(truncate_chars(&text, 1000));
        }
    };

    for raw in markdown.lines() {
        let line = raw.trim();
        if let Some(heading) = line.strip_prefix('#') {
            flush(&mut paragraph, &section, &mut intent);
            let title = heading.trim_start_matches('#').trim();
            if first_heading.is_none() {
                first_heading = Some(sanitize_single_line(title));
                "__first__".clone_into(&mut section);
            } else {
                section = title.to_ascii_lowercase();
            }
            continue;
        }
        let item = line
            .strip_prefix("- [")
            .or_else(|| line.strip_prefix("* ["))
            .and_then(|rest| {
                let (mark, text) = rest.split_once(']')?;
                matches!(mark, " " | "x" | "X").then_some(text.trim())
            });
        if let Some(text) = item {
            flush(&mut paragraph, &section, &mut intent);
            let text = truncate_chars(&sanitize_single_line(text), MAX_CRITERION_CHARS);
            if !text.is_empty() && criteria.len() < MAX_CRITERIA {
                criteria.push(Criterion {
                    id: CriterionId::parse(format!("ac_{}", criteria.len() + 1))?,
                    text,
                    status: CriterionStatus::NotChecked,
                    note: None,
                    references: Vec::new(),
                });
            }
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = ["task:", "ticket:", "issue:"]
            .iter()
            .find_map(|key| lower.starts_with(key).then(|| line[key.len()..].trim()))
        {
            if task_ref.is_none() && !value.is_empty() {
                task_ref = Some(truncate_chars(&sanitize_single_line(value), 200));
            }
            continue;
        }
        if line.is_empty() {
            flush(&mut paragraph, &section, &mut intent);
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(line);
        }
    }
    flush(&mut paragraph, &section, &mut intent);
    // A brief with only a title still says what it is about.
    let intent = intent.or(first_heading.filter(|h| !h.is_empty()));
    Ok(Brief {
        intent_summary: intent,
        task_ref,
        acceptance_criteria: criteria,
        source_copy: Some(markdown.to_owned()),
    })
}

/// Adds a criterion and returns its id.
pub fn add_criterion(brief: &mut Brief, text: &str) -> Result<CriterionId> {
    let text = truncate_chars(&sanitize_single_line(text), MAX_CRITERION_CHARS);
    if text.is_empty() {
        return Err(Error::invalid("criterion", "the text is empty"));
    }
    if brief.acceptance_criteria.len() >= MAX_CRITERIA {
        return Err(Error::invalid(
            "criterion",
            format!("at most {MAX_CRITERIA} criteria"),
        ));
    }
    let next = brief
        .acceptance_criteria
        .iter()
        .filter_map(|c| c.id.as_str().strip_prefix("ac_")?.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = CriterionId::parse(format!("ac_{next}"))?;
    brief.acceptance_criteria.push(Criterion {
        id: id.clone(),
        text,
        status: CriterionStatus::NotChecked,
        note: None,
        references: Vec::new(),
    });
    Ok(id)
}

/// Sets the status of a criterion. `Verified` is a statement of the human, never inferred.
pub fn set_criterion_status(
    brief: &mut Brief,
    id: &CriterionId,
    status: CriterionStatus,
    note: Option<&str>,
) -> Result<()> {
    let criterion = brief
        .acceptance_criteria
        .iter_mut()
        .find(|c| &c.id == id)
        .ok_or_else(|| Error::not_found("criterion", id))?;
    criterion.status = status;
    if let Some(note) = note {
        let note = sanitize_single_line(note);
        criterion.note = (!note.is_empty()).then_some(note);
    }
    Ok(())
}

/// Sets the intent summary (an empty text clears it).
pub fn set_intent(brief: &mut Brief, text: &str) {
    let text = truncate_chars(&sanitize_single_line(text), 1000);
    brief.intent_summary = (!text.is_empty()).then_some(text);
}

/// One reviewable unit as coverage sees it.
#[derive(Debug, Clone)]
pub struct CoverageUnit {
    /// Identity of the unit.
    pub id: UnitId,
    /// Its file.
    pub file: RepoPath,
    /// The module of the file.
    pub module: String,
    /// Its priority.
    pub priority: Priority,
    /// A semantic comparison was possible (otherwise it is counted as `unknown`).
    pub semantic: bool,
}

/// How much of the change a human has inspected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Units (or files) counted in the denominator.
    pub total: usize,
    /// Units (or files) the human marked.
    pub reviewed: usize,
    /// Units without semantic support; they still count in `total`.
    pub unknown: usize,
    /// `(reviewed, total)` by priority.
    pub by_priority: BTreeMap<Priority, (usize, usize)>,
    /// `(reviewed, total)` by module.
    pub by_module: BTreeMap<String, (usize, usize)>,
    /// Units of priority `P0`/`P1` not yet reviewed, in the order given.
    pub pending_critical: Vec<UnitId>,
}

impl Coverage {
    /// The raw fraction `reviewed / total`; 1.0 when there is nothing to review.
    #[allow(clippy::cast_precision_loss)]
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.reviewed as f32 / self.total as f32
        }
    }
}

/// Computes coverage. Only explicit marks count: `reviewed` is the set the human toggled.
pub fn compute_coverage(units: &[CoverageUnit], reviewed: &[UnitId], count: CountUnit) -> Coverage {
    let marked: HashSet<&UnitId> = reviewed.iter().collect();
    let mut coverage = Coverage::default();
    match count {
        CountUnit::Symbol => {
            for unit in units {
                let done = marked.contains(&unit.id);
                tally(
                    &mut coverage,
                    &unit.module,
                    unit.priority,
                    done,
                    !unit.semantic,
                );
                if !done && unit.priority.is_high() {
                    coverage.pending_critical.push(unit.id.clone());
                }
            }
        }
        CountUnit::File => {
            let mut files: BTreeMap<&RepoPath, Vec<&CoverageUnit>> = BTreeMap::new();
            for unit in units {
                files.entry(&unit.file).or_default().push(unit);
            }
            for group in files.values() {
                let done = group.iter().all(|u| marked.contains(&u.id));
                let priority = group
                    .iter()
                    .map(|u| u.priority)
                    .min()
                    .unwrap_or(Priority::P3);
                let module = &group[0].module;
                tally(
                    &mut coverage,
                    module,
                    priority,
                    done,
                    group.iter().any(|u| !u.semantic),
                );
                if !done && priority.is_high() {
                    coverage.pending_critical.extend(
                        group
                            .iter()
                            .filter(|u| !marked.contains(&u.id) && u.priority.is_high())
                            .map(|u| u.id.clone()),
                    );
                }
            }
        }
    }
    coverage
}

fn tally(coverage: &mut Coverage, module: &str, priority: Priority, done: bool, unknown: bool) {
    coverage.total += 1;
    coverage.reviewed += usize::from(done);
    coverage.unknown += usize::from(unknown);
    let by_priority = coverage.by_priority.entry(priority).or_default();
    by_priority.1 += 1;
    by_priority.0 += usize::from(done);
    let by_module = coverage.by_module.entry(module.to_owned()).or_default();
    by_module.1 += 1;
    by_module.0 += usize::from(done);
}

/// Non-blocking notes shown before finishing a review. They never prevent anything.
pub fn finish_warnings(
    coverage: &Coverage,
    brief: &Brief,
    linked_tests_missing: usize,
    warn_on_pending_critical: bool,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if warn_on_pending_critical && !coverage.pending_critical.is_empty() {
        warnings.push(format!(
            "{} P0/P1 unit(s) are not marked as reviewed",
            coverage.pending_critical.len()
        ));
    }
    let open = brief
        .acceptance_criteria
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                CriterionStatus::NotChecked | CriterionStatus::Pending
            )
        })
        .count();
    if open > 0 {
        warnings.push(format!("{open} acceptance criteria are still unchecked"));
    }
    if linked_tests_missing > 0 {
        warnings.push(format!("{linked_tests_missing} changed symbol(s) have no associated test (which does not prove there is none)"));
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRIEF: &str = "# Refactor authentication\n\nTask: PROJ-123\n\nMove token validation into its own module without changing the HTTP contract.\nKeep error codes.\n\n## Acceptance criteria\n\n- [ ] Preserve HTTP status codes\n- [x] Tokens are validated in one place\n* [ ] No new dependencies\n\n## Notes\n\nSome notes that are not the intent.\n";

    #[test]
    fn a_markdown_brief_is_read_without_trusting_its_checkmarks() {
        let brief = parse_brief(BRIEF).unwrap();
        assert_eq!(
            brief.intent_summary.as_deref(),
            Some(
                "Move token validation into its own module without changing the HTTP contract. Keep error codes."
            )
        );
        assert_eq!(brief.task_ref.as_deref(), Some("PROJ-123"));
        assert_eq!(brief.acceptance_criteria.len(), 3);
        assert_eq!(brief.acceptance_criteria[0].id.as_str(), "ac_1");
        assert!(
            brief
                .acceptance_criteria
                .iter()
                .all(|c| c.status == CriterionStatus::NotChecked),
            "importing never verifies"
        );
        assert_eq!(brief.source_copy.as_deref(), Some(BRIEF));
    }

    #[test]
    fn small_and_hostile_briefs() {
        assert_eq!(
            parse_brief("# Just a title\n")
                .unwrap()
                .intent_summary
                .as_deref(),
            Some("Just a title")
        );
        assert_eq!(parse_brief("").unwrap().intent_summary, None);
        let hostile =
            parse_brief("# T\n\n- [ ] do \x1b[31mthis\x1b[0m\x1b]52;c;ZXZpbA==\x07 now\n").unwrap();
        assert_eq!(hostile.acceptance_criteria[0].text, "do this now");
        assert!(parse_brief(&"x".repeat(MAX_BRIEF_BYTES + 1)).is_err());
        let many = (0..150).fold(String::new(), |mut acc, i| {
            use std::fmt::Write as _;
            let _ = writeln!(acc, "- [ ] item {i}");
            acc
        });
        assert_eq!(
            parse_brief(&many).unwrap().acceptance_criteria.len(),
            MAX_CRITERIA
        );
    }

    #[test]
    fn criteria_are_edited_by_the_human() {
        let mut brief = Brief::default();
        let first = add_criterion(&mut brief, "  keeps\n  codes ").unwrap();
        let second = add_criterion(&mut brief, "no new deps").unwrap();
        assert_eq!((first.as_str(), second.as_str()), ("ac_1", "ac_2"));
        assert_eq!(brief.acceptance_criteria[0].text, "keeps codes");
        set_criterion_status(
            &mut brief,
            &first,
            CriterionStatus::Verified,
            Some("checked routes.py"),
        )
        .unwrap();
        assert_eq!(
            brief.acceptance_criteria[0].status,
            CriterionStatus::Verified
        );
        assert_eq!(
            brief.acceptance_criteria[0].note.as_deref(),
            Some("checked routes.py")
        );
        assert!(
            set_criterion_status(
                &mut brief,
                &CriterionId::parse("ac_9").unwrap(),
                CriterionStatus::Pending,
                None
            )
            .is_err()
        );
        assert!(add_criterion(&mut brief, "   ").is_err());
        set_intent(&mut brief, "  goal ");
        assert_eq!(brief.intent_summary.as_deref(), Some("goal"));
        set_intent(&mut brief, "");
        assert_eq!(brief.intent_summary, None);
    }

    fn unit(
        id: &str,
        file: &str,
        module: &str,
        priority: Priority,
        semantic: bool,
    ) -> CoverageUnit {
        CoverageUnit {
            id: UnitId::parse(format!("unit_{id}")).unwrap(),
            file: RepoPath::new(file).unwrap(),
            module: module.into(),
            priority,
            semantic,
        }
    }

    fn units() -> Vec<CoverageUnit> {
        vec![
            unit("a", "src/api/x.py", "src/api", Priority::P0, true),
            unit("b", "src/api/x.py", "src/api", Priority::P2, true),
            unit("c", "src/db/y.py", "src/db", Priority::P1, true),
            unit("d", "README.md", "<root>", Priority::P3, false),
        ]
    }

    #[test]
    fn coverage_counts_only_explicit_marks() {
        let reviewed = [
            UnitId::parse("unit_b").unwrap(),
            UnitId::parse("unit_d").unwrap(),
        ];
        let coverage = compute_coverage(&units(), &reviewed, CountUnit::Symbol);
        assert_eq!(
            (coverage.reviewed, coverage.total, coverage.unknown),
            (2, 4, 1)
        );
        assert_eq!(coverage.by_priority[&Priority::P0], (0, 1));
        assert_eq!(coverage.by_priority[&Priority::P2], (1, 1));
        assert_eq!(coverage.by_module["src/api"], (1, 2));
        assert_eq!(
            coverage
                .pending_critical
                .iter()
                .map(UnitId::as_str)
                .collect::<Vec<_>>(),
            vec!["unit_a", "unit_c"]
        );
        assert!((coverage.fraction() - 0.5).abs() < f32::EPSILON);
        assert_eq!(
            compute_coverage(&[], &[], CountUnit::Symbol).fraction(),
            1.0
        );
    }

    #[test]
    fn counting_by_file_needs_every_unit_of_the_file() {
        let partly = [UnitId::parse("unit_a").unwrap()];
        let coverage = compute_coverage(&units(), &partly, CountUnit::File);
        assert_eq!(coverage.total, 3);
        assert_eq!(coverage.reviewed, 0, "x.py has an unreviewed unit");
        let all = [
            UnitId::parse("unit_a").unwrap(),
            UnitId::parse("unit_b").unwrap(),
        ];
        assert_eq!(
            compute_coverage(&units(), &all, CountUnit::File).reviewed,
            1
        );
        assert_eq!(
            coverage.by_priority[&Priority::P0].1,
            1,
            "a file takes its most urgent unit's priority"
        );
    }

    #[test]
    fn warnings_never_block_and_stay_honest() {
        let coverage = compute_coverage(&units(), &[], CountUnit::Symbol);
        let mut brief = Brief::default();
        add_criterion(&mut brief, "one").unwrap();
        let warnings = finish_warnings(&coverage, &brief, 3, true);
        assert_eq!(warnings.len(), 3);
        assert!(warnings[0].contains("2 P0/P1"));
        assert!(warnings[2].contains("does not prove"));
        assert!(finish_warnings(&coverage, &Brief::default(), 0, false).is_empty());
    }
}
