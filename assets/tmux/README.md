# Embedded tmux assets

`crates/klaub-tmux/build.rs` looks for `assets/tmux/<target-triple>/` while compiling.
When it contains `tmux.gz` and `manifest.json`, the executable is compiled into `klaub`
(the `klaub_embedded_tmux` cfg) and verified against `manifest.json` before every use.

The directories are **build products and are not committed**. Create one with:

```shell
scripts/build-tmux.sh                      # host target
scripts/build-tmux.sh x86_64-unknown-linux-gnu
```

CI does this for the release targets (see `.github/workflows/tmux-asset.yml`): `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-apple-darwin`.
Set `KLAUB_TMUX_ASSET_DIR` to point the build at an asset directory elsewhere.
Design and trade-offs: [ADR 0004](../../docs/adr/0004-embedded-verified-tmux.md).
