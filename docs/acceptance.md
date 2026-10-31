# Acceptance

The specification (§22) ends with a list of manual acceptance tests. Most of them are now covered by an automated test; the rest need a person with a real agent. This page says which is which, so "all green" does not claim more than it checked.

## Run by CI

| Specification check | Where it is asserted |
| --- | --- |
| Switch five times between the agent and the reviewer without losing the agent's history | `crates/klaub-tmux/tests/phase0_gate.rs` (`switching_windows_five_times_keeps_the_harness_and_its_history`), on Linux and macOS with the embedded tmux |
| Paste without `Enter`, as a bracketed single line, onto existing input, into a replaced or vanished pane | the same file |
| Read a new or rewritten file by symbol, with both versions in one viewport | `crates/klaub-app/tests/session.rs` (`the_reader_shows_both_versions_new_files_and_change_regions`), `crates/klaub-parse/tests` |
| Toggle before/after in one viewport | `crates/klaub-tui/tests/reader.rs` (`toggle_before_after`) |
| Navigate from a changed function to a known route, seeing the confidence | `crates/klaub-graph/tests`, `crates/klaub-app/tests/session.rs` (`the_queue_puts_the_dangerous_change_first_and_explains_it`) |
| Understand why each item has its priority | `crates/klaub-rules/tests`, `crates/klaub-tui/tests/reader.rs` |
| Register comments, close, reopen, lose nothing | `crates/klaub-app/tests/session.rs` (`opening_twice_resumes_the_same_review_and_keeps_comments`), `crates/klaub-storage/tests` |
| Finish without pasting, then choose "Prepare in the agent", with a separate confirmation | `crates/klaub-tui/tests/reader.rs` (`finish_never_sends_enter`) |
| Recognise a changed worktree and refresh without silently moving comments | `crates/klaub-tui/tests/reader.rs` (`drift_banner`), `crates/klaub-app/tests/session.rs` (`drift_is_noticed_and_refresh_relocates_without_silent_updates`) |
| Brief and criteria, marked verified by the human only | `crates/klaub-context`, `crates/klaub-app/tests/session.rs` (`a_brief_file_is_imported_and_edited_by_the_human`) |
| Tests associated with a symbol, `stale` and `not_run`, absence of association not read as absence of tests | `crates/klaub-app/tests/session.rs` (`test_evidence_never_confuses_no_association_with_no_tests`, `imported_reports_turn_not_run_into_passed_or_stale`), `fixtures/reports` |
| An introduced risk and a pre-existing one, shown apart | `crates/klaub-rules/tests`, `crates/klaub-app/tests/metrics.rs` |
| Second round: decide a previous comment by hand, history intact | `crates/klaub-app/tests/session.rs` (`the_next_round_carries_comments_over_and_asks_for_a_human_decision`, `removing_the_commented_symbol_makes_the_anchor_stale_not_misplaced`) |
| Finish with P1 units unreviewed, with a warning that does not block | `crates/klaub-app/tests/session.rs` (`coverage_counts_marks_and_warns_without_blocking`) |
| A valid flow, navigable to real code; a missing one prompts without `Enter`; a stale, malformed, escaping or control-character one is refused | `fixtures/flows` (`crates/klaub-flow/tests/fixtures.rs`), `crates/klaub-app/tests/session.rs` (`the_flow_diagram_is_current_stale_invalid_or_missing_and_always_optional`), `crates/klaub-storage/tests` |
| Reload the flow without restarting the agent | the same flow test (`reload_flow`) |

## Not run by CI: needs a person

These are the checks the specification makes against the real harnesses (Codex CLI and Claude Code), terminals and SSH. Nothing in this repository drives those programs, so none of them has been run by an automated test. Tick them on a machine you trust before you rely on the behaviour, and say in the pull request which versions you used.

- [ ] `klaub wrap codex` and `klaub wrap -- claude`: the agent starts, accepts keys, shows colours and follows a resize.
- [ ] Switch to the reviewer and back five times; the agent's scrollback and running task are intact.
- [ ] Prepare feedback in the agent: the one-line instruction appears in the input, **no Enter was sent**, and it can be edited before you submit it.
- [ ] Prepare while the agent's input already has text: nothing is lost and the confirmation warned you.
- [ ] Prepare while the agent is busy answering: the text waits in the input and nothing is submitted.
- [ ] Detach the tmux session (`Prefix d`) and `klaub session attach`: both windows are where they were.
- [ ] In `Ghostty`, `WezTerm`, `iTerm2`, `Kitty` or `Tabby` (the specification names them as targets, not as guarantees): the reader draws, the function keys arrive, bracketed paste works.
- [ ] Over a real SSH connection: `klaub doctor` says so, the reader works, and Show file is the way to read the feedback ([ssh.md](ssh.md)).
- [ ] With a real project of your own: the time to the first queue feels right. The numbers in [benchmarks.md](benchmarks.md) are for a synthetic corpus.
- [ ] With a real agent and the `klaub-flow` skill installed (`klaub skills install flow --harness codex`): ask for a diagram, wait, press `L`, and navigate it.
