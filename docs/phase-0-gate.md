# Phase 0 gate

The specification (§23) makes phase 0 a **gate**: if safe paste and session persistence do not hold on the target harnesses, the integration is fixed before any semantic feature is built on it. This page records what the automated suite proves, and what only a human with the real harnesses can.

## What the test suite proves

The tests in [`crates/klaub-tmux/tests/phase0_gate.rs`](../crates/klaub-tmux/tests/phase0_gate.rs) run against a **real tmux server** on a private socket. The harness is a stand-in program that puts the terminal in non-canonical mode, enables bracketed paste and records every byte it receives, so the assertions are about exact bytes.

| Specification requirement | Test |
| --- | --- |
| Paste test text with bracketed paste, **without Enter** | `text_is_pasted_as_a_bracketed_single_line_and_never_submitted` |
| Nothing is submitted when the harness ignores bracketed paste | `without_bracketed_paste_support_no_line_break_can_submit_anything` |
| Existing input is neither cleared nor overwritten | `existing_input_is_neither_cleared_nor_overwritten` |
| Address the pane by id; refuse a replaced process | `a_replaced_process_in_the_agent_pane_is_refused` |
| Refuse a vanished pane; end the session cleanly | `a_vanished_agent_pane_is_refused_and_the_session_is_reaped` |
| Switch to the reviewer five times and back without losing the harness or its history | `switching_windows_five_times_keeps_the_harness_and_its_history` |
| The review window can be recreated without touching the agent | `a_closed_review_window_is_recreated_without_touching_the_agent` |
| The user's tmux is never involved; sessions are isolated | `ending_one_session_never_touches_another`, `the_private_server_ignores_the_users_tmux_configuration` |

The CLI end-to-end tests in [`crates/klaub-cli/tests/cli.rs`](../crates/klaub-cli/tests/cli.rs) add: refusing to nest inside tmux, validating the command before starting anything, and the `wrap` → `list` → `kill` lifecycle.

The private server binds these prefix chords (default prefix `Ctrl+B`). `Prefix+R` and `Prefix+A` only switch windows. The others select the review window and then send a function key to its pane.

| Chord | Effect |
| --- | --- |
| Prefix+R | Focus the review window |
| Prefix+A | Focus the agent window |
| Prefix+I | Impact (`F5`) |
| Prefix+C | Comments (`F6`) |
| Prefix+T | Tests (`F7`) |
| Prefix+B | Brief / context (`F8`) |
| Prefix+L | Flow (`F9`) |
| Prefix+d | Detach |
| Prefix+? | Show the chord list |

CI runs the suite on Linux (tmux from the distribution) and macOS (Homebrew). The [tmux asset workflow](../.github/workflows/tmux-asset.yml) additionally builds the pinned tmux for each release target and repeats a `wrap`/`list`/`kill` smoke test with that tmux **embedded** and no tmux on the search path.

## What still needs a human

The suite cannot run Codex CLI or Claude Code. Before calling phase 0 closed for a release, run this checklist with each real harness and note the result in the pull request:

| Step | Codex CLI | Claude Code |
| --- | :---: | :---: |
| `klaub wrap <harness>` starts it and it is fully usable (keys, colours, resize) | ☐ | ☐ |
| `Ctrl+B R` shows the review window; `Ctrl+B A` returns; repeat five times; the conversation is intact | ☐ | ☐ |
| In the review window press `q`, then `Ctrl+B R`: the window is recreated | ☐ | ☐ |
| Detach (`Ctrl+B d`), run `klaub session attach`, the harness is as it was left | ☐ | ☐ |
| From the review window, run `klaub review`'s paste action (phase 1) or, until then, exercise `TmuxAgentChannel::prepare_input`: the text appears in the input box as one pasted block and **is not submitted** | ☐ | ☐ |
| With half-typed text in the input box, the pasted text is appended, nothing is erased | ☐ | ☐ |
| Closing the harness ends the session; `klaub session list` shows no running session | ☐ | ☐ |

If any box cannot be ticked, open an issue labelled `phase-0` with the harness version and `klaub doctor` output.
