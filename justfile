# Developer tasks. Run `just` to list them. CI calls cargo directly, `just ci` mirrors it.

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Format all code.
fmt:
    cargo fmt --all

# Run every check that CI runs, in the order of cheapest first.
ci: fmt-check clippy test doc deny typos

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Run unit and integration tests (integration tests need git and tmux on PATH).
test:
    cargo test --workspace --all-features --locked

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked

deny:
    cargo deny check

typos:
    typos

# Coverage report in target/llvm-cov/html (needs cargo-llvm-cov).
coverage:
    cargo llvm-cov --workspace --all-features --html

# Run the CLI from source: `just run review --help`.
run *ARGS:
    cargo run -p klaub -- {{ARGS}}
