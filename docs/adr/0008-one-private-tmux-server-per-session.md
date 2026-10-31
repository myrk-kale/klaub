# 8. One private tmux server per Klaub session

- Status: accepted
- Date: 2026-10-31
- Refines: [ADR 0004](0004-embedded-verified-tmux.md)

## Context

[ADR 0004](0004-embedded-verified-tmux.md) chose a dedicated socket label so Klaub never shares a server with the user's tmux. It left open whether all Klaub sessions share one such server. Building the wrapper showed three reasons not to:

- A tmux server takes its environment from the process that started it. Sessions created later inside a shared server would run the agent with the *first* session's `PATH` and variables, not the current shell's.
- Key bindings and hooks are server-wide. `Prefix R` must mean "review window of *this* session", which a shared server can only express with extra indirection.
- Ending a session should be able to mean "stop the server", never "find our sessions among others". The strongest guarantee that Klaub cannot affect anything it did not create is that each server holds exactly one session.

## Decision

Every `klaub wrap` starts its own tmux server on the socket label `klaub-<session-id>` (empty configuration, `TMUX` removed from the environment of every call). The session id, pane ids and the process id of the agent are recorded in `<state>/sessions/<session-id>.json`. `kill-server` is only ever issued against that socket.

## Consequences

- Isolation is physical: two Klaub sessions cannot see or influence each other (covered by `ending_one_session_never_touches_another`).
- There is one tmux process per active session, which is negligible next to the agent itself.
- `klaub session list` is driven by the registry and verified against each server; a record whose server is gone is reported as `ended`.
