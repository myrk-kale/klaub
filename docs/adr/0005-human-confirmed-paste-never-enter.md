# 5. Human-confirmed paste, never `Enter`

- Status: accepted
- Date: 2026-10-31

## Context

The review ends by handing feedback to an agent. tmux cannot reliably tell whether a harness's composer is empty, ready, or in the middle of a prompt, so any automatic submission risks sending half-typed text or overwriting the user's input.

## Decision

`Finish review` persists the feedback and stops there. A separate action, `Prepare in agent`, asks for confirmation (showing the destination pane, the exact text and a warning about possible existing input), then pastes with `paste-buffer -p` (bracketed paste) into the recorded pane id and returns to the agent window. Klaub never sends `Enter`, never clears the composer, and falls back to *Copy feedback* or *Show file* when the pane identity cannot be proven. Text is sanitised first: control bytes, ANSI and OSC sequences are removed and the length is bounded. Comments are treated as untrusted data in the pasted instruction, which only references the review file.

## Consequences

- The user always presses `Enter` themselves.
- The `submitted` comment state is set by the human, because a paste proves nothing about submission.
- Auto-submit stays out of scope until a harness-specific readiness contract exists (see the `HarnessAdapter` extension point in `docs/architecture.md`).
