# Benchmarks

Latency has not been measured. There is no p50 and no p95, and no [criterion](https://github.com/bheisler/criterion.rs) benchmark exists in the workspace. The reason is the same for both: there is no representative corpus yet. A number taken from the handful of files the tests build would describe the fixture, not Klaub, so none is published here.

## What is measured, and how

The four figures the specification asks for are measured by test, not by benchmark. Each one is an assertion against a Git repository whose correct answer is known, so a regression fails the test instead of quietly moving a number. They live in [`crates/klaub-app/tests/metrics.rs`](../crates/klaub-app/tests/metrics.rs).

| Metric | Test | What it asserts |
| --- | --- | --- |
| Symbol-match precision and recall | `symbol_match_precision_one_of_one_recall_one_of_one_on_four_functions` | One rename, one body edit, one addition: all three classified as that kind, and the untouched function absent. |
| Temporal classification | `one_swallowed_error_is_introduced_and_none_preexist` | An `except` block that only discards the error, added to a function that had none, is one `introduced` finding and zero `preexisting`. |
| Symbol-to-test association | `a_calling_test_associates_and_an_uncalled_function_does_not` | A test that imports and calls a function is associated with it; a function nothing calls has no association. |
| Comment-anchor stability | `a_comment_anchor_follows_its_function_when_lines_are_inserted_above_it` | A comment survives an edit that pushes its function down, and the anchor line moves with it. |

Run them with:

```
cargo test -p klaub-app --features klaub-git/testing --test metrics
```

## Matching thresholds

Symbol matching treats a score of `0.85` or above as a strong match and `0.65` or above as a probable one (`STRONG_MATCH` and `PROBABLE_MATCH` in `crates/klaub-core/src/semantic.rs`, exposed as `analysis.match_strong_threshold` and `analysis.match_probable_threshold`). They have not been tuned. The only evidence for them is the fixture above, which is four functions in one file, and that is too small to justify changing either value.
