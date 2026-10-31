# Releasing

Releases are produced by the [`release` workflow](.github/workflows/release.yml) when a tag `vX.Y.Z` is pushed.

## Checklist

1. Make sure `main` is green.
2. Move the *Unreleased* entries in [CHANGELOG.md](CHANGELOG.md) under a new `## [X.Y.Z] - YYYY-MM-DD` heading and update the comparison links.
3. Bump `workspace.package.version` in `Cargo.toml` and run `cargo update -p klaub-cli --precise X.Y.Z` (or simply `cargo check`) so `Cargo.lock` follows.
4. Open a PR `chore(release): vX.Y.Z`, wait for CI, merge.
5. Tag and push: `git tag -s vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z`.

## What the workflow does

1. Builds tmux for each target from a pinned, SHA-256-verified source tarball and records a manifest with the SHA-256 of the executable.
2. Builds `klaub` with that tmux embedded (`cargo build --release --locked`).
3. Packages `klaub-<version>-<target>.tar.gz` containing the binary, `LICENSE`, `THIRD-PARTY-NOTICES.md` (tmux, libevent, ncurses), and a `.sha256` file.
4. Generates build-provenance attestations for every archive.
5. Creates the GitHub release with notes extracted from the changelog.

A dry run is available from the *Actions* tab (`workflow_dispatch`, input `dry_run=true`); it builds and uploads artifacts without publishing.

## Targets

Linux x86_64 (glibc baseline of the runner image) and macOS arm64. Linux aarch64 and macOS x86_64 are added when verifiable CI runners and binaries exist.
