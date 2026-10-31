# 9. The pasted instruction is always a single line

- Status: accepted
- Date: 2026-10-31
- Refines: [ADR 0005](0005-human-confirmed-paste-never-enter.md)

## Context

`tmux paste-buffer -p` wraps the text in bracketed-paste markers (`ESC[200~ … ESC[201~`) **only when the application has enabled bracketed paste**, and tmux exposes no format variable that says whether it has. Inside the payload tmux also converts every line feed into a carriage return, which is what the `Enter` key sends. Observed on tmux 3.7:

```
ESC[200~line one\rline two ESC[201~     (application enabled bracketed paste)
line one\rline two                      (application did not)
```

In the second case a multi-line paste would *submit* the first line. Detecting the application's mode from outside is not reliable, and ADR 0005 forbids any behaviour that could submit input.

## Decision

`prepare_input` reduces the text to one line before it reaches tmux: terminal control and escape sequences are removed, every run of line breaks and tabs becomes one space, and the length is bounded to 4000 characters. A line without a carriage return cannot be submitted whether or not the markers are honoured. Instructions that Klaub generates are written as one short paragraph that points at the review file, which remains the source of truth for the comments.

## Consequences

- The guarantee "Klaub never presses `Enter`" holds even with harnesses that ignore bracketed paste (covered by `without_bracketed_paste_support_no_line_break_can_submit_anything`).
- Multi-line prompts cannot be pasted. The generated instructions never need them.
