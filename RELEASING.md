# Releasing

Releases are produced by the [`release` workflow](.github/workflows/release.yml) when a tag `vX.Y.Z` is pushed.

## Checklist

1. Make sure `main` is green.
2. Move the *Unreleased* entries in [CHANGELOG.md](CHANGELOG.md) under a new `## [X.Y.Z] - YYYY-MM-DD` heading and update the comparison links.
3. Bump `version` in `Cargo.toml` and run `cargo update -p klaub --precise X.Y.Z` (or simply `cargo check`) so `Cargo.lock` follows.
4. Open a PR `chore(release): vX.Y.Z`, wait for CI, merge.
5. Tag and push: `git tag -s vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z`.

## What the workflow does

1. Builds tmux for each target from a pinned, SHA-256-verified source tarball and records a manifest with the SHA-256 of the executable.
2. Builds `klaub` with that tmux embedded (`cargo build --release --locked`).
3. Packages `klaub-<version>-<target>.tar.gz` containing the binary, `LICENSE`, `THIRD-PARTY-NOTICES.md` (tmux, libevent, ncurses), and a `.sha256` file.
4. Generates build-provenance attestations for every archive.
5. Creates the GitHub release with notes extracted from the changelog.
6. Dispatches the `crates.io` workflow, which publishes the crate.

A dry run is available from the *Actions* tab (`workflow_dispatch`, input `dry_run=true`); it builds and uploads artifacts without publishing.

## Targets

Linux x86_64 and aarch64 (glibc baseline of the runner image) and macOS arm64 and x86_64.

## Publishing to crates.io

**Pending.** Nothing is published to crates.io yet, and the release workflow does not dispatch the publish until the repository variable `CRATES_IO_PUBLISH` is `true`. To turn it on: configure the Trusted Publishers below, set the variable (`gh variable set CRATES_IO_PUBLISH --body true`), then run the workflow once for the latest tag (`gh workflow run crates.yml -f tag=vX.Y.Z`). After the first publish, change the README install line from `cargo install --git ...` to `cargo install klaub --locked`.

`.github/workflows/crates.yml` publishes the `klaub` crate with `cargo publish`, using crates.io Trusted Publishing: GitHub's OIDC token is exchanged for a short-lived token, so no secret is stored. It runs on its own, dispatched by the release workflow for the tag, and it can be run by hand for an existing tag (`gh workflow run crates.yml -f tag=vX.Y.Z`). A version that is already on crates.io is skipped.

One-time setup, done by the crate owner in the crates.io web interface: Settings, Trusted Publishing, add a GitHub publisher with owner `myrk-kale`, repository `klaub`, workflow `crates.yml` and no environment. A crate that was never published can be given a pending publisher before its first release.

A build from crates.io does not embed tmux, because the embedded binaries are built by the release pipeline and are not part of the package. It needs `--tmux-path` or a tmux on `PATH`; the GitHub release archives and the Homebrew formula carry the embedded one.

The `package` job of CI runs `cargo package` on every pull request, so a change that only builds inside the repository fails there.
