# Security model

Klaub runs on a developer's machine, next to a coding agent, over repositories the developer may not fully trust. This document states what Klaub assumes, what it defends against, and what it deliberately does not claim.

## Assets

1. The developer's source code and review comments.
2. The integrity of the developer's terminal and of the agent's input.
3. The developer's other processes: the agent, their own tmux server, their shell.

## Trust boundaries

| Input | Trust | Handling |
| --- | --- | --- |
| Source files, file names, Git output | Untrusted | Parsed with fault-tolerant grammars; control sequences stripped before display |
| Review comments (also in later rounds) | Untrusted data | Never executed or interpolated into commands; the pasted instruction only references the review file |
| Flow documents written by an agent | Untrusted | Strict schema, size limits, relative paths only, symlink-escape checks, references checked against the snapshot |
| Brief and test report files | Untrusted | Read only when the user names them; same path validation |
| Embedded tmux binary | Trusted after verification | SHA-256 checked against the manifest before every execution |
| Environment, `.klaub.toml` of the project | Semi-trusted | A project cannot cause command execution; the test command is only run after explicit confirmation of the exact command line |

## Controls

- **No shell.** Subprocesses (`git`, `tmux`, test commands) are spawned with argument arrays. Test selections are validated and never start with `-`.
- **Terminal injection.** All text that reaches the screen or a pane passes through `klaub_core::text::sanitize_terminal_text`, which removes C0/C1 controls, ANSI CSI and OSC sequences and bidirectional override characters.
- **Pane identity.** The agent pane is recorded by tmux pane id and process id. Before pasting, Klaub re-checks that the pane exists, is not dead and still has the recorded process. Otherwise it offers *Copy feedback* or *Show file*.
- **No automatic submission.** Klaub never sends `Enter` and never clears the composer ([ADR 0005](adr/0005-human-confirmed-paste-never-enter.md)).
- **Private tmux server.** Dedicated socket label, empty user configuration, verified private binary ([ADR 0004](adr/0004-embedded-verified-tmux.md)). Klaub never runs `kill-server` against a socket it does not own.
- **Filesystem.** Cache directories are created with mode `0700`; extracted executables are written to a temporary file and renamed; reads follow symlinks only inside the worktree.
- **Privacy.** There is no network code, no telemetry and no API key storage. Logs contain identifiers and counts, not code or comments, unless the user raises the log level explicitly.

## Non-goals and known limits

- Klaub is not a sandbox. Running the configured test command executes project code with the user's privileges. The command line and working directory are shown for confirmation, and timeouts and cancellation apply, but nothing is isolated.
- A paste cannot prove that the harness's composer is empty or ready. The confirmation dialog says so.
- Priorities and findings are heuristics for ordering human attention, not vulnerability reports.

## Reporting

See [SECURITY.md](../SECURITY.md).
