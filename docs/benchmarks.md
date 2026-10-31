# Benchmarks

Two harnesses time the pipeline against the latency targets the specification sets in §19. They are in the workspace and they run on a **synthetic corpus**, so what they report is the shape of Klaub's work on a repository of a given size, on one machine. Neither number is a promise about your project, and the specification itself calls its targets "hypotheses of engineering, not measured numbers of the product". Nothing is enforced: a target that is missed is printed as `over`, not turned into a failing test, because the figure depends on the machine.

## Latency

```
cargo bench -p klaub-app --bench latency        # analysis: first queue, toggle, refresh
cargo bench -p klaub-tui --bench ui             # the reader: one key press plus its frame
KLAUB_BENCH_SIZES=small,medium,large cargo bench -p klaub-app --bench latency
```

Both are plain `harness = false` programs that print a table of p50, p95 and max, with the specification's target next to each row and whether the p95 is within it. The `klaub-app` harness builds a Git repository of Python files whose functions call the next one and a function in the next file, commits it, rewrites a share of the files, and times:

- the first queue with a cold parse cache and with a warm one (the time to open a review);
- the work behind the before/after toggle, for every unit in both directions;
- a new snapshot after a one-file edit (`R`), with the other parses cached.

The `klaub-tui` harness drives the real reader over 200 files with 60 changed, draws into a 120×40 `TestBackend`, and times a key press plus the frame it causes while moving in the queue, moving in the structure tree and toggling before/after.

### One run

Apple M4, macOS 26.2, Rust 1.99.1, release profile, synthetic corpus, 2026-10-31. Times are milliseconds. This is one run on one laptop, kept here so a regression is visible, not as a claim about other hardware.

| Corpus | Operation | n | p50 | p95 | max | Specification (§19) |
| --- | --- | --- | --- | --- | --- | --- |
| 20 files, 8 changed | first queue, cold cache | 5 | 68.69 | 82.49 | 82.49 | 2 s for a small or medium review: within |
| 20 files, 8 changed | first queue, warm cache | 9 | 66.10 | 66.83 | 66.83 | no target |
| 20 files, 8 changed | toggle before/after (view rebuild) | 80 | 0.05 | 0.05 | 0.07 | p95 50 ms: within |
| 20 files, 8 changed | refresh after a one-file edit | 10 | 65.14 | 68.83 | 68.83 | p95 3 s: within |
| 200 files, 60 changed | first queue, cold cache | 5 | 96.56 | 97.93 | 97.93 | 2 s for a small or medium review: within |
| 200 files, 60 changed | first queue, warm cache | 9 | 84.34 | 86.05 | 86.05 | no target |
| 200 files, 60 changed | toggle before/after (view rebuild) | 1200 | 0.09 | 0.09 | 0.10 | p95 50 ms: within |
| 200 files, 60 changed | refresh after a one-file edit | 10 | 86.05 | 89.88 | 89.88 | p95 3 s: within |

| Operation (200 files, 60 changed, 120×40) | n | p50 | p95 | max | Specification (§19) |
| --- | --- | --- | --- | --- | --- |
| move in the queue (key + frame) | 300 | 11.97 | 13.13 | 19.66 | p95 100 ms: within |
| move in the structure tree (key + frame) | 300 | 12.69 | 14.41 | 19.74 | p95 100 ms: within |
| toggle before/after (key + frame) | 300 | 0.22 | 0.45 | 0.79 | p95 50 ms: within |

### What this does not tell you

- **The cold and warm queue times are about the same.** The files in this corpus are tiny, so parsing is a small share of opening a review, and the cache cannot show its worth. Where the rest of the time goes (capturing the snapshot through the Git CLI, building the call graph, scoring) was not profiled.
- **No real project was measured.** A monorepo with large files, deep call graphs and a slow disk will move every one of these rows. The `large` size (`KLAUB_BENCH_SIZES=large`) exists to see the slope, not to predict a project.
- **The specification's other rows have no harness.** "tmux window switch" is the tmux server's own work, not Klaub's, and "comment recovery after a crash" is a durability property that the storage tests assert (atomic writes) rather than a latency.
- **p95 over five or ten samples is close to the maximum.** The slow rows (open, refresh) take a few seconds to repeat, so they have few samples. The key-press rows have 300.

## What is measured by test, and how

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
