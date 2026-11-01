# Fixtures

Small, hand-written inputs that tests read from disk. They live here, outside any crate, so that the
files a reviewer would hand to Klaub are plain files you can open.

| Folder | What it holds | Read by |
| --- | --- | --- |
| `flows/` | Flow documents of the kind the `klaub-flow` skill writes: one that is current, one written for another snapshot, one whose symbol does not exist, and four that must be rejected (malformed JSON, a path that escapes the worktree, terminal control characters in a note, an unknown schema version). | `tests/flow_fixtures.rs` |
| `reports/` | A JUnit XML report with one failing test and a Coverage.py XML report. | `tests/test_evidence_fixtures.rs` |

Repositories used by the other tests are built on the fly by `klaub::adapters::git::testing::TempRepo`, because
a Git repository cannot live inside another one as plain files.

File names say what is expected: `current*`, `stale*`, `symbol-missing*` and `invalid-*`.
