# Benchmarks

Two harnesses time the pipeline against Klaub's latency targets: 2 s to open a small or medium review, 100 ms for a key press in a list or tree, 50 ms for the before/after toggle, 3 s to refresh. They are in the repository (`benches/`) and they run on a **synthetic corpus**, so what they report is the shape of Klaub's work on a repository of a given size, on one machine. Neither number is a promise about your project, and the targets are engineering hypotheses to be measured, not promises. Nothing is enforced: a target that is missed is printed as `over`, not turned into a failing test, because the figure depends on the machine.

## Latency

```
cargo bench --bench latency        # analysis: first queue, toggle, refresh
cargo bench --bench ui             # the reader: one key press plus its frame
KLAUB_BENCH_SIZES=small,medium,large cargo bench --bench latency
```

Both are plain `harness = false` programs that print a table of p50, p95 and max, with the target next to each row and whether the p95 is within it. The `latency` harness builds a Git repository of Python files whose functions call the next one and a function in the next file, commits it, rewrites a share of the files, and times:

- the first queue with a cold parse cache and with a warm one (the time to open a review);
- the work behind the before/after toggle, for every unit in both directions;
- a new snapshot after a one-file edit (`R`), with the other parses cached.

The `ui` harness drives the real reader over 200 files with 60 changed, draws into a 120×40 `TestBackend`, and times a key press plus the frame it causes while moving in the queue, moving in the structure tree and toggling before/after.

### One run

Apple M4, macOS 26.2, Rust 1.99.1, release profile, synthetic corpus, 2026-10-31. Times are milliseconds. This is one run on one laptop, kept here so a regression is visible, not as a claim about other hardware.

| Corpus | Operation | n | p50 | p95 | max | Target |
| --- | --- | --- | --- | --- | --- | --- |
| 20 files, 8 changed | first queue, cold cache | 5 | 68.69 | 82.49 | 82.49 | 2 s for a small or medium review: within |
| 20 files, 8 changed | first queue, warm cache | 9 | 66.10 | 66.83 | 66.83 | no target |
| 20 files, 8 changed | toggle before/after (view rebuild) | 80 | 0.05 | 0.05 | 0.07 | p95 50 ms: within |
| 20 files, 8 changed | refresh after a one-file edit | 10 | 65.14 | 68.83 | 68.83 | p95 3 s: within |
| 200 files, 60 changed | first queue, cold cache | 5 | 96.56 | 97.93 | 97.93 | 2 s for a small or medium review: within |
| 200 files, 60 changed | first queue, warm cache | 9 | 84.34 | 86.05 | 86.05 | no target |
| 200 files, 60 changed | toggle before/after (view rebuild) | 1200 | 0.09 | 0.09 | 0.10 | p95 50 ms: within |
| 200 files, 60 changed | refresh after a one-file edit | 10 | 86.05 | 89.88 | 89.88 | p95 3 s: within |

| Operation (200 files, 60 changed, 120×40) | n | p50 | p95 | max | Target |
| --- | --- | --- | --- | --- | --- |
| move in the queue (key + frame) | 300 | 11.97 | 13.13 | 19.66 | p95 100 ms: within |
| move in the structure tree (key + frame) | 300 | 12.69 | 14.41 | 19.74 | p95 100 ms: within |
| toggle before/after (key + frame) | 300 | 0.22 | 0.45 | 0.79 | p95 50 ms: within |

### Real repositories

`KLAUB_BENCH_REPO` points the same `latency` harness at real repositories (a comma-separated list of paths), and `KLAUB_BENCH_BASE` names the revision to compare `HEAD` against (default `HEAD~30`). The repository is cloned into a temporary directory first, so nothing is written into yours, and the review is the branch scope, which is read-only: there is no refresh row, because that needs an edit.

```
KLAUB_BENCH_REPO=~/src/flask,~/src/django KLAUB_BENCH_BASE=HEAD~60 \
  cargo bench --bench latency
```

One run, same machine as above (Apple M4, macOS 26.2, Rust 1.99.1, release profile, 2026-10-31). Each repository was cloned with `--depth 80` and compared `HEAD~60` against `HEAD`. Times are milliseconds. The commits are the ones the clones had on that day, so the run can be repeated on the same inputs.

| Repository (commit) | Operation | n | p50 | p95 | max | Target |
| --- | --- | --- | --- | --- | --- | --- |
| flask `49943b5a94`: 236 files, 94 changed, 369 units | first queue, cold cache | 3 | 259.90 | 264.15 | 264.15 | 2 s: within |
| | first queue, warm cache | 5 | 212.52 | 215.21 | 215.21 | no target |
| | toggle before/after | 738 | 0.85 | 6.56 | 7.10 | p95 50 ms: within |
| tokio `b499257ef6`: 890 files, 149 changed, 630 units | first queue, cold cache | 3 | 459.02 | 460.03 | 460.03 | 2 s: within |
| | first queue, warm cache | 5 | 283.22 | 290.57 | 290.57 | no target |
| | toggle before/after | 1000 | 0.61 | 3.40 | 6.85 | p95 50 ms: within |
| vuejs/core `24c9d0bffb`: 703 files, 115 changed, 331 units | first queue, cold cache | 3 | 1128.41 | 1129.15 | 1129.15 | 2 s: within |
| | first queue, warm cache | 5 | 570.54 | 582.12 | 582.12 | no target |
| | toggle before/after | 662 | 1.02 | 7.82 | 15.94 | p95 50 ms: within |
| django `a05e16a723`: 7085 files, 266 changed, 560 units | first queue, cold cache | 3 | 2082.87 | 2156.35 | 2156.35 | 2 s: **over** |
| | first queue, warm cache | 5 | 1124.20 | 1134.36 | 1134.36 | no target |
| | toggle before/after | 1000 | 0.79 | 8.23 | 40.96 | p95 50 ms: within |

Django has seven thousand files, which is not the "small or medium review" the 2 s target is written for, so the `over` is reported rather than argued away.

The first run of this harness on these four repositories found three defects that the synthetic corpus could not, and they are fixed in the same change:

- **A crash on tokio.** A function that moved from one file to another made the analysis index the wrong file's symbol table and panic with an out-of-bounds index. On a small repository the same bug did not crash; it quietly gave the contract and the "before" code of the wrong symbol. Both sides of a symbol reference are now read from the file the reference names, and the "before" code of a moved symbol is shown from the file it left (`a_symbol_moved_to_another_file_*` in `tests/session.rs`).
- **One `git` process per project file.** Reviewing a branch or a range of commits read every project file with its own `git cat-file`. Reading them in batches took tokio from 3.1 s to 0.46 s and vue core from 2.9 s to 1.1 s. The default scope, the worktree, reads from disk and was not affected.
- **A quadratic test association.** Linking tests to changed symbols compared every changed symbol with every test case, repeating a per-file import check each time. It now checks a file's imports once per target and file. Together with the batch read, this took Django from 20 s to 2.1 s.

### What this does not tell you

- **The cold and warm queue times are about the same.** The files in this corpus are tiny, so parsing is a small share of opening a review, and the cache cannot show its worth. Where the rest of the time goes (capturing the snapshot through the Git CLI, building the call graph, scoring) was not profiled.
- **Four open-source repositories were measured, on one disk.** A monorepo with large files, deep call graphs and a slow or networked disk will move every one of these rows. The shallow clones also keep only 80 commits, so the diffs are 60 commits of history, not a feature branch someone reviews. The `large` synthetic size (`KLAUB_BENCH_SIZES=large`) exists to see the slope.
- **Two other targets have no harness.** Switching between the agent and the reviewer is the terminal multiplexer's own work, not Klaub's, and "comment recovery after a crash" is a durability property that the storage tests assert (atomic writes) rather than a latency.
- **p95 over five or ten samples is close to the maximum.** The slow rows (open, refresh) take a few seconds to repeat, so they have few samples. The key-press rows have 300.

## What is measured by test, and how

Four figures are measured by test, not by benchmark. Each one is an assertion against a Git repository whose correct answer is known, so a regression fails the test instead of quietly moving a number. They live in [`tests/metrics.rs`](../tests/metrics.rs).

| Metric | Test | What it asserts |
| --- | --- | --- |
| Symbol-match precision and recall | `symbol_match_precision_one_of_one_recall_one_of_one_on_four_functions` | One rename, one body edit, one addition: all three classified as that kind, and the untouched function absent. |
| Temporal classification | `one_swallowed_error_is_introduced_and_none_preexist` | An `except` block that only discards the error, added to a function that had none, is one `introduced` finding and zero `preexisting`. |
| Symbol-to-test association | `a_calling_test_associates_and_an_uncalled_function_does_not` | A test that imports and calls a function is associated with it; a function nothing calls has no association. |
| Comment-anchor stability | `a_comment_anchor_follows_its_function_when_lines_are_inserted_above_it` | A comment survives an edit that pushes its function down, and the anchor line moves with it. |

Run them with:

```
cargo test --test metrics
```

## Matching thresholds

Symbol matching treats a score of `0.85` or above as a strong match and `0.65` or above as a probable one (`STRONG_MATCH` and `PROBABLE_MATCH` in `src/domain/semantic.rs`, exposed as `analysis.match_strong_threshold` and `analysis.match_probable_threshold`). They have not been tuned. The only evidence for them is the fixture above, which is four functions in one file, and that is too small to justify changing either value.
