# Third-party notices

Klaub is distributed under the [MIT License](LICENSE). Release archives also contain
software written by others. Its licence texts are in the `licenses/` directory of the archive.

## Embedded programs (release builds)

Release builds of `klaub` embed one executable built from the following sources, pinned
by version and SHA-256 in [`scripts/build-tmux.sh`](scripts/build-tmux.sh):

| Component | Version | Licence | Notes |
| --- | --- | --- | --- |
| [tmux](https://github.com/tmux/tmux) | 3.8 | ISC | The terminal multiplexer Klaub drives. Run from a private, hash-verified copy; never installed system-wide. |
| [libevent](https://libevent.org) | 2.1.13-stable | BSD-3-Clause (with some files under other BSD licences) | Statically linked into tmux. |
| [ncurses](https://invisible-island.net/ncurses/) | 6.6 | X11-style (MIT-like) | Statically linked into tmux. Terminal descriptions are read from the system's terminfo database. |
| [utf8proc](https://github.com/JuliaStrings/utf8proc) | 2.12.0 | MIT (with Unicode data under the Unicode licence) | macOS builds only: correct character widths for emoji and complex code points. |
| [jemalloc](https://github.com/jemalloc/jemalloc) | 5.4.0 | BSD-2-Clause | macOS builds only: tmux asks for it because the system `calloc` has been seen not to zero memory. |

`dependencies.txt` in the archive lists the system libraries the embedded tmux still loads at run time.

## Rust dependencies

Klaub is built from crates published on [crates.io](https://crates.io). Their exact versions
are pinned in [`Cargo.lock`](Cargo.lock), and CI (`cargo deny check licenses`) rejects any
dependency whose licence is not in the allow list in [`deny.toml`](deny.toml)
(MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, BSL-1.0).
