//! Readers for existing test reports: JUnit XML and Coverage.py XML (Cobertura).
//!
//! Reports are untrusted input: entity expansion is limited to the predefined XML entities, the
//! size is bounded, messages are truncated and stripped of terminal sequences, and file names
//! must be valid repository paths.

use crate::domain::path::RepoPath;
use crate::domain::testing::{CoverageEntry, TestOutcome, TestResult};
use crate::domain::text::{sanitize_single_line, truncate_chars};
use crate::domain::{Error, Result};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

/// Largest report that is read.
pub(crate) const MAX_REPORT_BYTES: usize = 32 * 1024 * 1024;
const MAX_MESSAGE_CHARS: usize = 300;

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(std::borrow::Cow::into_owned)
}

fn checked(xml: &str) -> Result<()> {
    if xml.len() > MAX_REPORT_BYTES {
        Err(Error::invalid(
            "report",
            format!("larger than {MAX_REPORT_BYTES} bytes"),
        ))
    } else {
        Ok(())
    }
}

fn xml_error(error: &quick_xml::Error) -> Error {
    Error::invalid("report", format!("not well-formed XML: {error}"))
}

fn begin_testcase(element: &BytesStart<'_>) -> TestResult {
    let name = attribute(element, "name").unwrap_or_default();
    let class = attribute(element, "classname").unwrap_or_default();
    let full = if class.is_empty() {
        name
    } else {
        format!("{class}::{name}")
    };
    TestResult {
        name: sanitize_single_line(&full),
        file: attribute(element, "file").and_then(|f| RepoPath::new(f).ok()),
        outcome: TestOutcome::Passed,
        duration_ms: attribute(element, "time")
            .and_then(|t| t.parse::<f64>().ok())
            .filter(|t| t.is_finite() && *t >= 0.0)
            .map(|t| {
                // Bounded by the filter above and by what a test run can plausibly take.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let millis = (t * 1000.0).round() as u64;
                millis
            }),
        message: None,
    }
}

fn apply_outcome(result: &mut TestResult, element: &BytesStart<'_>) {
    match element.name().as_ref() {
        "failure" | "error" => {
            result.outcome = TestOutcome::Failed;
            result.message = attribute(element, "message")
                .map(|m| truncate_chars(&sanitize_single_line(&m), MAX_MESSAGE_CHARS));
        }
        "skipped" => result.outcome = TestOutcome::Skipped,
        _ => {}
    }
}

/// Parses a JUnit XML report into one result per test case.
pub fn parse_junit(xml: &str) -> Result<Vec<TestResult>> {
    checked(xml)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut results = Vec::new();
    let mut current: Option<TestResult> = None;
    loop {
        match reader.read_event().map_err(|e| xml_error(&e))? {
            Event::Start(e) if e.name().as_ref() == "testcase" => {
                current = Some(begin_testcase(&e));
            }
            // A self-closing test case is complete on its own: it passed.
            Event::Empty(e) if e.name().as_ref() == "testcase" => results.push(begin_testcase(&e)),
            Event::Start(e) | Event::Empty(e) => {
                if let Some(result) = current.as_mut() {
                    apply_outcome(result, &e);
                }
            }
            Event::End(e) if e.name().as_ref() == "testcase" => results.extend(current.take()),
            Event::Eof => break,
            _ => {}
        }
    }
    results.extend(current);
    Ok(results)
}

/// Parses a Coverage.py XML (Cobertura) report into per-file executed and missed lines.
pub fn parse_coverage_xml(xml: &str) -> Result<Vec<CoverageEntry>> {
    checked(xml)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut entries: Vec<CoverageEntry> = Vec::new();
    let mut current: Option<CoverageEntry> = None;
    loop {
        match reader.read_event().map_err(|e| xml_error(&e))? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                "class" => {
                    if let Some(entry) = current.take() {
                        entries.push(entry);
                    }
                    current = attribute(&e, "filename")
                        .and_then(|f| RepoPath::new(f).ok())
                        .map(|file| CoverageEntry {
                            file,
                            covered_lines: Vec::new(),
                            missed_lines: Vec::new(),
                        });
                }
                "line" => {
                    if let (Some(entry), Some(number), Some(hits)) = (
                        current.as_mut(),
                        attribute(&e, "number").and_then(|n| n.parse::<u32>().ok()),
                        attribute(&e, "hits").and_then(|h| h.parse::<u64>().ok()),
                    ) {
                        if hits > 0 {
                            entry.covered_lines.push(number);
                        } else {
                            entry.missed_lines.push(number);
                        }
                    }
                }
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == "class" => entries.extend(current.take()),
            Event::Eof => break,
            _ => {}
        }
    }
    entries.extend(current);
    // The same file can appear in several <class> elements: merge them.
    entries.sort_by(|a, b| a.file.cmp(&b.file));
    let mut merged: Vec<CoverageEntry> = Vec::new();
    for entry in entries {
        match merged.last_mut() {
            Some(last) if last.file == entry.file => {
                last.covered_lines.extend(entry.covered_lines);
                last.missed_lines.extend(entry.missed_lines);
            }
            _ => merged.push(entry),
        }
    }
    for entry in &mut merged {
        entry.covered_lines.sort_unstable();
        entry.covered_lines.dedup();
        entry.missed_lines.sort_unstable();
        entry.missed_lines.dedup();
        entry
            .missed_lines
            .retain(|l| entry.covered_lines.binary_search(l).is_err());
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUNIT: &str = r#"<?xml version="1.0"?>
<testsuites>
  <testsuite name="pytest" tests="4">
    <testcase classname="tests.test_auth.TestAuth" name="test_login" time="0.012"/>
    <testcase classname="tests.test_auth.TestAuth" name="test_logout" time="1.5">
      <failure message="assert 1 == 2&#10;red">traceback</failure>
    </testcase>
    <testcase classname="tests.test_auth" name="test_skip"><skipped message="not now"/></testcase>
    <testcase name="test_error"><error message="boom"/></testcase>
  </testsuite>
</testsuites>"#;

    #[test]
    fn junit_results() {
        let results = parse_junit(JUNIT).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].name, "tests.test_auth.TestAuth::test_login");
        assert_eq!(results[0].outcome, TestOutcome::Passed);
        assert_eq!(results[0].duration_ms, Some(12));
        assert_eq!(results[1].outcome, TestOutcome::Failed);
        assert_eq!(
            results[1].message.as_deref(),
            Some("assert 1 == 2 red"),
            "newlines and escapes are gone"
        );
        assert_eq!(results[2].outcome, TestOutcome::Skipped);
        assert_eq!(results[3].outcome, TestOutcome::Failed);
        assert_eq!(results[3].name, "test_error");
    }

    #[test]
    fn junit_is_defensive() {
        assert!(
            parse_junit("<testsuite><testcase name=\"a\"").is_err()
                || parse_junit("<testsuite><testcase name=\"a\"")
                    .unwrap()
                    .len()
                    <= 1
        );
        assert_eq!(parse_junit("<testsuites/>").unwrap(), vec![]);
        assert!(parse_junit(&"x".repeat(MAX_REPORT_BYTES + 1)).is_err());
        let hostile = "<testsuite><testcase classname=\"../../etc\" name=\"x\" file=\"../../etc/passwd\" time=\"-5\"/></testsuite>";
        let results = parse_junit(hostile).unwrap();
        assert_eq!(
            results[0].file, None,
            "paths that escape the repository are dropped"
        );
        assert_eq!(results[0].duration_ms, None);
        let bomb = r#"<!DOCTYPE x [<!ENTITY a "aaaa"><!ENTITY b "&a;&a;&a;&a;">]><testsuite><testcase name="&b;"/></testsuite>"#;
        let _ = parse_junit(bomb); // must neither expand nor hang
    }

    const COVERAGE: &str = r#"<?xml version="1.0" ?>
<coverage version="7.4" line-rate="0.8">
  <packages><package name="app"><classes>
    <class name="auth.py" filename="src/app/auth.py" line-rate="0.5">
      <lines><line number="1" hits="1"/><line number="2" hits="0"/><line number="3" hits="4"/></lines>
    </class>
    <class name="auth_extra" filename="src/app/auth.py">
      <lines><line number="2" hits="2"/><line number="9" hits="0"/></lines>
    </class>
    <class name="odd" filename="/abs/elsewhere.py"><lines><line number="1" hits="1"/></lines></class>
  </classes></package></packages>
</coverage>"#;

    #[test]
    fn coverage_is_merged_per_file() {
        let entries = parse_coverage_xml(COVERAGE).unwrap();
        assert_eq!(entries.len(), 1, "absolute paths are dropped");
        assert_eq!(entries[0].file.as_str(), "src/app/auth.py");
        assert_eq!(
            entries[0].covered_lines,
            vec![1, 2, 3],
            "line 2 was executed by one of the classes"
        );
        assert_eq!(entries[0].missed_lines, vec![9]);
        assert_eq!(entries[0].line_rate(), Some(0.75));
    }

    #[test]
    fn malformed_coverage_is_an_error() {
        assert!(parse_coverage_xml("<coverage><class filename='a.py'></coverage>").is_err());
        assert!(parse_coverage_xml("<coverage/>").unwrap().is_empty());
    }
}
