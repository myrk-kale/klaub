//! Embeds a pre-built tmux when one is available for the compilation target.
//!
//! The release pipeline (`scripts/build-tmux.sh`) writes
//! `assets/tmux/<target>/{tmux.gz,manifest.json}`. When both files exist they are
//! compiled into the binary and the `klaub_embedded_tmux` cfg is set; otherwise
//! the crate builds without tmux and requires `--tmux-path` at run time
//! (see docs/adr/0004-embedded-verified-tmux.md).

#![allow(clippy::print_stdout)]

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(klaub_embedded_tmux)");
    println!("cargo:rerun-if-env-changed=KLAUB_TMUX_ASSET_DIR");
    println!("cargo:rerun-if-changed=build.rs");

    let Ok(target) = env::var("TARGET") else {
        return;
    };
    let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") else {
        return;
    };

    let dir = env::var_os("KLAUB_TMUX_ASSET_DIR").map_or_else(
        || {
            PathBuf::from(manifest_dir)
                .join("../../assets/tmux")
                .join(&target)
        },
        PathBuf::from,
    );
    let gz = dir.join("tmux.gz");
    let manifest = dir.join("manifest.json");

    // Registering the paths even when absent makes cargo rebuild once they appear.
    println!("cargo:rerun-if-changed={}", gz.display());
    println!("cargo:rerun-if-changed={}", manifest.display());

    if gz.is_file()
        && manifest.is_file()
        && let (Ok(gz), Ok(manifest)) = (gz.canonicalize(), manifest.canonicalize())
    {
        println!("cargo:rustc-cfg=klaub_embedded_tmux");
        println!("cargo:rustc-env=KLAUB_TMUX_GZ={}", gz.display());
        println!("cargo:rustc-env=KLAUB_TMUX_MANIFEST={}", manifest.display());
    }
}
