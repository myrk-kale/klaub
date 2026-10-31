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

## The allocator on macOS

tmux asks for jemalloc on macOS, because the system `calloc` has been seen not to zero memory. On `x86_64-apple-darwin` a tmux built with jemalloc 5.4 crashes with a segmentation fault in the forked child, before it can exec the pane's command (`crashed on child side of fork pre-exec`), with and without jemalloc's zone allocator. That was observed on the `macos-15-intel` runner and only there: the same source works on Apple Silicon. The Intel asset is therefore built with the system allocator (`scripts/build-tmux.sh` decides by target), and `KLAUB_TMUX_MALLOC=jemalloc|jemalloc-nozone|system` overrides it for an experiment. If you can reproduce or explain the jemalloc crash, the Intel build can go back to the default.
