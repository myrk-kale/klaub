# Releasing

Releases are produced by the [`release` workflow](.github/workflows/release.yml) when a tag `vX.Y.Z` is pushed.

## Checklist

1. Make sure `main` is green.
2. Move the *Unreleased* entries in [CHANGELOG.md](CHANGELOG.md) under a new `## [X.Y.Z] - YYYY-MM-DD` heading and update the comparison links.
3. Bump `workspace.package.version` in `Cargo.toml` and run `cargo update -p klaub --precise X.Y.Z` (or simply `cargo check`) so `Cargo.lock` follows.
4. Open a PR `chore(release): vX.Y.Z`, wait for CI, merge.
5. Tag and push: `git tag -s vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z`.

## What the workflow does

1. Builds tmux for each target from a pinned, SHA-256-verified source tarball and records a manifest with the SHA-256 of the executable.
2. Builds `klaub` with that tmux embedded (`cargo build --release --locked`).
3. Packages `klaub-<version>-<target>.tar.gz` containing the binary, `LICENSE`, `THIRD-PARTY-NOTICES.md` (tmux, libevent, ncurses), and a `.sha256` file.
4. Generates build-provenance attestations for every archive.
5. Creates the GitHub release with notes extracted from the changelog.
6. Dispatches the `crates.io` workflow, which publishes every crate of the workspace.

A dry run is available from the *Actions* tab (`workflow_dispatch`, input `dry_run=true`); it builds and uploads artifacts without publishing.

## Targets

Linux x86_64 and aarch64 (glibc baseline of the runner image) and macOS arm64 and x86_64.

## Publishing to crates.io

`.github/workflows/crates.yml` publishes the 16 crates of the workspace with `cargo publish --workspace`, using crates.io Trusted Publishing: GitHub's OIDC token is exchanged for a short-lived token, so no secret is stored. It runs on its own, dispatched by the release workflow for the tag, and it can be run by hand for an existing tag (`gh workflow run crates.yml -f tag=vX.Y.Z`). A version that is already on crates.io is skipped.

One-time setup, done by a crate owner in the crates.io web interface, for each crate (`klaub`, `klaub-app`, `klaub-context`, `klaub-core`, `klaub-flow`, `klaub-git`, `klaub-graph`, `klaub-match`, `klaub-parse`, `klaub-review`, `klaub-rules`, `klaub-storage`, `klaub-tests`, `klaub-tmux`, `klaub-tui`): Settings, Trusted Publishing, add a GitHub publisher with owner `myrk-kale`, repository `klaub`, workflow `crates.yml` and no environment. A crate that was never published can be given a pending publisher before its first release.

The `package` job of CI runs `cargo package --workspace` on every pull request, so a crate that cannot be built from its own archive fails there.
