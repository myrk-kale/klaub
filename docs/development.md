# Development guide

## Prerequisites

- Rust **1.90** or newer (`rustup` recommended). The repository pins the *stable* channel in `rust-toolchain.toml`.
- `git` 2.30+ and `tmux` 3.2+ on `PATH`. Integration tests create throwaway Git repositories and talk to a real tmux server on a private socket; they skip with a message when `tmux` is missing.
- Optional: [`just`](https://github.com/casey/just), [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny), [`typos`](https://github.com/crate-ci/typos), [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov).

## Everyday commands

```shell
just ci          # fmt check, clippy, tests, docs, cargo-deny, typos
just test        # cargo test --workspace
just run review  # run the CLI from source
```

Without `just`, the equivalents are in the `justfile`; they are plain `cargo` invocations.

## Running with a system tmux

Development builds do not embed tmux (see [ADR 0004](adr/0004-embedded-verified-tmux.md)). Point Klaub at one explicitly:

```shell
export KLAUB_TMUX_PATH="$(command -v tmux)"
cargo run -p klaub-cli -- doctor
```

### Embedded tmux

Release builds embed a hash-verified tmux. To produce one locally, run `scripts/build-tmux.sh` and then `cargo build --release`. The script **downloads and compiles tmux, libevent and ncurses** from pinned, SHA-256-verified tarballs and writes `assets/tmux/<target>/`; CI does the same on disposable runners (`.github/workflows/tmux-asset.yml`), so you rarely need to. `klaub doctor` shows whether the binary in use is embedded and verified or an explicit path.

### Trying a wrapped session from source

```shell
export KLAUB_TMUX_PATH="$(command -v tmux)"
cargo build -p klaub-cli
target/debug/klaub wrap -- sh        # any interactive program works as a stand-in agent
# Ctrl+B R  review window   Ctrl+B A  back to the agent   Ctrl+B d  detach
target/debug/klaub session list
target/debug/klaub session kill --yes <id>
```

Set `KLAUB_STATE_DIR`, `KLAUB_CACHE_DIR` and `KLAUB_CONFIG_DIR` to keep experiments out of your real directories. `KLAUB_LOG=debug` writes a log file for the review window.

## Conventions

- **Commits and PR titles** follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(tui): ...`, `fix(git): ...`. Scopes are crate names without the `klaub-` prefix.
- **Branches**: `feat/...`, `fix/...`, `docs/...`, `chore/...`; merged through pull requests.
- **Errors**: library crates return `klaub_core::Result`; only `klaub-cli` uses `anyhow`.
- **No panics in library code**: `unwrap` and `expect` are linted outside tests. Prefer `?` and explicit error variants.
- **No `println!` in libraries**; use `tracing`. The CLI writes results to stdout and everything else to stderr. `--format json` prints JSON only on stdout.
- **Untrusted input**: file names, source code, comments and flow documents are untrusted. Strip terminal control sequences before rendering or pasting (`klaub_core::text`), validate paths with `RepoPath`, and pass subprocess arguments as arrays, never through a shell.
- **Documentation**: every public item has a doc comment; `cargo doc` runs with warnings denied.

## Adding a port or an adapter

1. Define the trait and the value types it exchanges in `klaub-core::ports`; keep it free of adapter vocabulary.
2. Add an in-memory fake next to the tests that need it.
3. Implement the adapter in its own crate, depending only on `klaub-core`.
4. Wire it in `klaub-cli`'s composition root.
5. Run `cargo test -p klaub-cli --test architecture` to confirm the dependency rules still hold.

## Fixtures

Tests that need a repository build one on the fly with the helpers in `klaub-git`'s test support, so no `.git` directories are committed. Static source samples used by parser and matcher tests live under `fixtures/`.

## Releases

Releases are cut from tags. See [RELEASING.md](../RELEASING.md).
