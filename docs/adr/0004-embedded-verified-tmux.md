# 4. Embedded, verified tmux in a private server

- Status: accepted (the socket naming is refined by [ADR 0008](0008-one-private-tmux-server-per-session.md))
- Date: 2026-10-31

## Context

Klaub needs window and pane control that survives detach and attach, and a way to inject text into the harness's input. tmux provides both. The user's own tmux server, configuration and key bindings must not be affected, and Klaub must not run an unknown binary found on `PATH`.

## Decision

- Release builds embed a gzip-compressed tmux binary per target together with a manifest that records the tmux version and the SHA-256 of the decompressed executable. `crates/klaub-tmux/build.rs` embeds the asset when `assets/tmux/<target>/` exists; `scripts/build-tmux.sh` and the release workflow produce it from a pinned, hash-verified upstream tarball.
- On first use the binary is extracted to a private, versioned cache directory (mode `0700`, file mode `0700`), written to a temporary file and renamed atomically. Its SHA-256 is verified before every execution.
- Klaub talks to its own server through a dedicated socket label (`-L klaub`), an empty configuration (`-f /dev/null`) plus a generated configuration, and never calls a generic `kill-server` against another socket.
- A development build without an embedded asset refuses to guess: the user passes `--tmux-path` (or `KLAUB_TMUX_PATH`) explicitly. `klaub doctor` reports which binary is in use and why.

## Consequences

- "One executable per OS/architecture" holds for users, while tmux stays third-party C software with its own licence notices, shipped alongside.
- A tmux upgrade is a reviewed change to one pinned version and hash.
- Contributors need a system tmux for integration tests; CI installs it.
