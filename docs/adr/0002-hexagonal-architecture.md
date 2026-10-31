# 2. Hexagonal architecture in a Cargo workspace

- Status: accepted
- Date: 2026-10-31

## Context

The specification requires that "the core is independent from tmux and from the UI", so that a web view or an IDE integration can reuse the analyser later. It also lists many collaborators that are expensive or unsafe to exercise in unit tests: Git, tree-sitter, the file system, SQLite, tmux, the terminal and the clipboard.

## Decision

We apply the ports and adapters (hexagonal) pattern and make the boundaries physical by giving each part its own crate:

- **`klaub-core`** holds the domain model and the *ports* (Rust traits). It performs no I/O: no `std::fs`, `std::process`, `std::net` or `std::env`.
- **Domain services** (`klaub-match`, `klaub-graph`, `klaub-rules`, `klaub-tests`, `klaub-context`, `klaub-flow`, `klaub-review`) are pure algorithms over domain types. They depend on `klaub-core` (and `klaub-graph` for `klaub-rules`) only.
- **`klaub-app`** is the application layer. It orchestrates use cases through ports and never names a concrete adapter.
- **Driven adapters** implement ports against the outside world: `klaub-git`, `klaub-parse`, `klaub-storage`, `klaub-tmux`.
- **Driving adapters** call the application: `klaub-tui` and `klaub-cli`. `klaub-cli` is also the *composition root* where adapters are wired to ports.

The rules are executable. `crates/klaub-cli/tests/architecture.rs` reads `cargo metadata` and fails when a crate depends on something its layer does not allow, and scans `klaub-core` for forbidden I/O APIs.

## Consequences

- Domain logic is tested with plain values and in-memory fakes, without Git repositories or terminals.
- Adapters can be replaced (for example `gix` instead of the Git CLI) without touching the domain.
- There are more crates than a project of this size would otherwise have. Compile times stay low because crates build in parallel and change independently.
