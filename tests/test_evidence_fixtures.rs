//! The report files under `fixtures/reports`, read the way an imported report is read.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use klaub::domain::testing::TestOutcome;
use klaub::services::test_evidence::{parse_coverage_xml, parse_junit};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/reports")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_junit_fixture_has_one_pass_and_one_failure_with_its_message() {
    let results = parse_junit(&fixture("junit.xml")).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].outcome, TestOutcome::Passed);
    assert_eq!(results[1].outcome, TestOutcome::Failed);
    assert_eq!(results[1].message.as_deref(), Some("assert 200 == 401"));
    assert!(results[1].name.contains("test_login_rejects_bad_password"));
}

#[test]
fn the_coverage_fixture_separates_covered_from_missed_lines() {
    let entries = parse_coverage_xml(&fixture("coverage.xml")).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file.as_str(), "domain/auth.py");
    assert_eq!(entries[0].covered_lines, vec![1, 2, 3]);
    assert_eq!(entries[0].missed_lines, vec![4]);
}
