#!/usr/bin/env bash
#
# Builds a self-contained tmux for one target and writes the asset that
# crates/klaub-tmux/build.rs embeds into the klaub binary:
#
#   assets/tmux/<target>/tmux.gz           gzip-compressed, stripped executable
#   assets/tmux/<target>/manifest.json     tmux version, target and SHA-256 of the executable
#   assets/tmux/<target>/dependencies.txt  system libraries the executable links to
#   assets/tmux/<target>/licenses/         licence texts of tmux, libevent and ncurses
#
# libevent and ncurses are built from pinned, hash-verified tarballs and linked
# statically; only the platform's C library (and, on macOS, system libraries) stay dynamic.
#
# Intended for CI runners (see .github/workflows/tmux-asset.yml and release.yml). It
# downloads and compiles third-party source code, so run it locally only if you want that.
#
# Usage: scripts/build-tmux.sh [target-triple]   (default: the host triple of rustc)
#
# Updating a pin: change the version and SHA-256 below in one reviewed commit and
# let CI rebuild. Hashes come from the upstream release page.

set -euo pipefail

TMUX_VERSION="3.8"
TMUX_URL="https://github.com/tmux/tmux/releases/download/${TMUX_VERSION}/tmux-${TMUX_VERSION}.tar.gz"
TMUX_SHA256="e79c699c7e949dccd0a4a125e17b8d1261e311b16979dbc8eb34542f3966d82e"

LIBEVENT_VERSION="2.1.13-stable"
LIBEVENT_URL="https://github.com/libevent/libevent/releases/download/release-${LIBEVENT_VERSION}/libevent-${LIBEVENT_VERSION}.tar.gz"
LIBEVENT_SHA256="f7e9383b8c0baa81b687e5b5eecc01beefaf1b19b64151d95ed61647fe7a315c"

# macOS only: tmux insists on utf8proc (the system wcwidth is poor for emoji) and jemalloc
# (macOS calloc has been seen not to zero memory) and refuses to configure without a choice.
UTF8PROC_VERSION="2.12.0"
UTF8PROC_URL="https://github.com/JuliaStrings/utf8proc/releases/download/v${UTF8PROC_VERSION}/utf8proc-${UTF8PROC_VERSION}.tar.gz"
UTF8PROC_SHA256="a393fbef160835fb315bc3e91ba8d86f7a73a7cec9e6198b6c60b848b498bfeb"

JEMALLOC_VERSION="5.4.0"
JEMALLOC_URL="https://github.com/jemalloc/jemalloc/releases/download/${JEMALLOC_VERSION}/jemalloc-${JEMALLOC_VERSION}.tar.bz2"
JEMALLOC_SHA256="200776fac271093e7c2f21edd6d62657ecd2be578d9328633f2a86bfa6ef4f1d"

NCURSES_VERSION="6.6"
NCURSES_URL="https://ftp.gnu.org/pub/gnu/ncurses/ncurses-${NCURSES_VERSION}.tar.gz https://ftpmirror.gnu.org/ncurses/ncurses-${NCURSES_VERSION}.tar.gz https://invisible-mirror.net/archives/ncurses/ncurses-${NCURSES_VERSION}.tar.gz"
NCURSES_SHA256="355b4cbbed880b0381a04c46617b7656e362585d52e9cf84a67e2009b749ff11"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
out="${root}/assets/tmux/${target}"
work="$(mktemp -d)"
prefix="${work}/prefix"
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
trap 'rm -rf "${work}"' EXIT

log() { printf '\n==> %s\n' "$*"; }

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# Downloads from the first URL that answers with the right bytes. A URL may be a space-separated
# list of mirrors; the SHA-256 is checked for every one, so a mirror cannot change what is built.
fetch() { # urls sha256 destination
  local urls="$1" expected="$2" dest="$3" url actual
  for url in ${urls}; do
    log "downloading ${url}"
    if ! curl --fail --silent --show-error --location --retry 2 --connect-timeout 20 \
        --max-time 300 --output "${dest}" "${url}"; then
      echo "could not download ${url}; trying the next source" >&2
      continue
    fi
    actual="$(sha256_of "${dest}")"
    if [ "${actual}" = "${expected}" ]; then
      return 0
    fi
    echo "SHA-256 mismatch for ${url}" >&2
    echo "  expected ${expected}" >&2
    echo "  actual   ${actual}" >&2
  done
  echo "no source gave ${dest##*/} with the expected SHA-256" >&2
  exit 1
}

case "$(uname -s)" in
  Darwin)
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"
    cflags="-O2 -mmacosx-version-min=${MACOSX_DEPLOYMENT_TARGET}"
    ;;
  *)
    cflags="-O2"
    ;;
esac
export CFLAGS="${cflags} -fPIC"

mkdir -p "${prefix}" "${work}/src"
fetch "${LIBEVENT_URL}" "${LIBEVENT_SHA256}" "${work}/libevent.tar.gz"
fetch "${NCURSES_URL}" "${NCURSES_SHA256}" "${work}/ncurses.tar.gz"
fetch "${TMUX_URL}" "${TMUX_SHA256}" "${work}/tmux.tar.gz"
for name in libevent ncurses tmux; do
  mkdir -p "${work}/src/${name}"
  tar -xzf "${work}/${name}.tar.gz" -C "${work}/src/${name}" --strip-components=1
done

log "building libevent ${LIBEVENT_VERSION}"
(
  cd "${work}/src/libevent"
  ./configure --prefix="${prefix}" --disable-shared --enable-static --with-pic \
    --disable-openssl --disable-samples --disable-libevent-regress >/dev/null
  make -j"${jobs}" >/dev/null
  make install >/dev/null
)

log "building ncurses ${NCURSES_VERSION}"
(
  cd "${work}/src/ncurses"
  ./configure --prefix="${prefix}" \
    --without-shared --without-debug --without-ada --without-cxx --without-cxx-binding \
    --without-manpages --without-tests --without-progs --without-gpm \
    --enable-widec --enable-pc-files --with-pkg-config-libdir="${prefix}/lib/pkgconfig" \
    --with-terminfo-dirs=/etc/terminfo:/lib/terminfo:/usr/share/terminfo:/usr/lib/terminfo \
    --with-default-terminfo-dir=/usr/share/terminfo >/dev/null
  make -j"${jobs}" >/dev/null
  make install.libs install.includes >/dev/null
)

# macOS allocator: jemalloc, which tmux asks for there because the system calloc has been seen
# not to zero memory, or the system allocator. On x86_64 macOS a tmux built with jemalloc 5.4 dies
# with a segmentation fault in the forked child before it can exec the pane's command, with or
# without jemalloc's zone allocator (observed on the macos-15-intel runner; the same source works
# on Apple Silicon). That target therefore uses the system allocator. KLAUB_TMUX_MALLOC (jemalloc,
# jemalloc-nozone or system) overrides the choice for experiments.
case "${target}" in
  x86_64-apple-darwin) default_malloc=system ;;
  *) default_malloc=jemalloc ;;
esac
malloc_choice="${KLAUB_TMUX_MALLOC:-${default_malloc}}"
tmux_flags=(--disable-utf8proc)
if [ "$(uname -s)" = Darwin ]; then
  fetch "${UTF8PROC_URL}" "${UTF8PROC_SHA256}" "${work}/utf8proc.tar.gz"
  fetch "${JEMALLOC_URL}" "${JEMALLOC_SHA256}" "${work}/jemalloc.tar.bz2"
  mkdir -p "${work}/src/utf8proc" "${work}/src/jemalloc"
  tar -xzf "${work}/utf8proc.tar.gz" -C "${work}/src/utf8proc" --strip-components=1
  tar -xjf "${work}/jemalloc.tar.bz2" -C "${work}/src/jemalloc" --strip-components=1

  log "building utf8proc ${UTF8PROC_VERSION}"
  (
    cd "${work}/src/utf8proc"
    make -j"${jobs}" >/dev/null
    make install prefix="${prefix}" >/dev/null
    # Only the static library may be linked.
    rm -f "${prefix}"/lib/libutf8proc*.dylib
  )

  if [ "${malloc_choice}" = system ]; then
    tmux_flags=(--enable-utf8proc --disable-jemalloc)
  else
    jemalloc_flags=(--disable-shared --enable-static)
    if [ "${malloc_choice}" = jemalloc-nozone ]; then
      jemalloc_flags+=(--disable-zone-allocator)
    fi
    log "building jemalloc ${JEMALLOC_VERSION} (${malloc_choice})"
    (
      cd "${work}/src/jemalloc"
      ./configure --prefix="${prefix}" "${jemalloc_flags[@]}" >/dev/null
      make -j"${jobs}" build_lib_static >/dev/null
      make install_lib_static install_include >/dev/null
    )
    tmux_flags=(--enable-utf8proc --enable-jemalloc)
  fi
fi

log "building tmux ${TMUX_VERSION}"
(
  cd "${work}/src/tmux"
  export PKG_CONFIG_PATH="${prefix}/lib/pkgconfig"
  export LIBEVENT_CFLAGS="-I${prefix}/include"
  export LIBEVENT_LIBS="-L${prefix}/lib -levent_core"
  export LIBNCURSES_CFLAGS="-I${prefix}/include -I${prefix}/include/ncursesw"
  export LIBNCURSES_LIBS="-L${prefix}/lib -lncursesw"
  export LIBUTF8PROC_CFLAGS="-I${prefix}/include"
  export LIBUTF8PROC_LIBS="-L${prefix}/lib -lutf8proc"
  export JEMALLOC_CFLAGS="-I${prefix}/include"
  export JEMALLOC_LIBS="-L${prefix}/lib -ljemalloc"
  ./configure --prefix="${prefix}" "${tmux_flags[@]}" >/dev/null
  make -j"${jobs}" >/dev/null
)

binary="${work}/src/tmux/tmux"
strip "${binary}"
reported="$("${binary}" -V)"
echo "built: ${reported}"
if [ "${reported}" != "tmux ${TMUX_VERSION}" ]; then
  echo "unexpected version: ${reported}" >&2
  exit 1
fi

log "writing ${out}"
rm -rf "${out}"
mkdir -p "${out}/licenses"
gzip -9 --no-name --stdout "${binary}" > "${out}/tmux.gz"
digest="$(sha256_of "${binary}")"
cat > "${out}/manifest.json" <<JSON
{
  "schema_version": 1,
  "tmux_version": "${TMUX_VERSION}",
  "target": "${target}",
  "sha256": "${digest}"
}
JSON

# Inventory of what stays dynamic, as required by the distribution policy.
{
  echo "# Dynamic dependencies of tmux ${TMUX_VERSION} (${target})"
  case "$(uname -s)" in
    Darwin) otool -L "${binary}" | tail -n +2 ;;
    *)      ldd "${binary}" ;;
  esac
} > "${out}/dependencies.txt"

cp "${work}/src/tmux/COPYING"     "${out}/licenses/tmux-COPYING"
cp "${work}/src/libevent/LICENSE" "${out}/licenses/libevent-LICENSE"
cp "${work}/src/ncurses/COPYING"  "${out}/licenses/ncurses-COPYING"
if [ "$(uname -s)" = Darwin ]; then
  cp "${work}/src/utf8proc/LICENSE.md" "${out}/licenses/utf8proc-LICENSE.md"
  cp "${work}/src/jemalloc/COPYING"    "${out}/licenses/jemalloc-COPYING"
fi

echo
echo "tmux ${TMUX_VERSION} for ${target}"
echo "  sha256  ${digest}"
echo "  asset   ${out}"
cat "${out}/dependencies.txt"
