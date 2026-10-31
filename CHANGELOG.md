# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Phase 0** (the gate): `klaub wrap <command>` runs any interactive program in a private tmux session with an `agent` and a `review` window; `klaub session list|attach|kill`, `klaub doctor`, `klaub version`, `klaub completions`.
- `klaub-tmux`: verified embedded tmux (SHA-256 checked before every execution, private `0700` extraction), one private server per session, pane-identity checks, and a bracketed, single-line, never-`Enter` paste. Nine integration tests run against a real tmux server and assert the exact bytes the harness receives.
- `klaub-tui`: the review window's first screen (Elm-style state/view, tested with Ratatui's `TestBackend`, hostile text sanitised).
- Release pipeline: `scripts/build-tmux.sh` (pinned tmux 3.8, libevent 2.1.13, ncurses 6.6), the `tmux asset` and `Release` workflows (checksums, build-provenance attestations, changelog-based notes), `scripts/install.sh`.
- ADRs 0008 (one private server per session) and 0009 (the pasted instruction is one line).
- `klaub-core`: the I/O-free domain model (snapshots, symbols, semantic changes, findings, reviews, test evidence, flow contract), the configuration schema with line/column errors and layered merging, terminal-text sanitisation, and the ports every adapter implements.
- Executable architecture rules (`crates/klaub-cli/tests/architecture.rs`): dependency direction between layers and an I/O-free core are enforced in CI.
- Project foundation: Cargo workspace, lint policy, continuous integration, security scanning, community files, architecture documentation and decision records.
