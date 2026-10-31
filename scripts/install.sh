#!/bin/sh
#
# Installs the latest (or a chosen) klaub release for this machine.
#
#   curl -fsSL https://raw.githubusercontent.com/myrk-kale/klaub/main/scripts/install.sh | sh
#
# Environment:
#   KLAUB_VERSION      version to install, e.g. 0.1.0 (default: latest release)
#   KLAUB_INSTALL_DIR  where to put the binary (default: $HOME/.local/bin)
#
# The archive is verified against the published SHA-256 before anything is installed.

set -eu

repo="myrk-kale/klaub"
install_dir="${KLAUB_INSTALL_DIR:-$HOME/.local/bin}"

fail() { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "required tool not found: $1"; }

need curl
need tar
need uname

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)            target="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64|Darwin-aarch64) target="aarch64-apple-darwin" ;;
  *) fail "no prebuilt klaub for $(uname -s) $(uname -m); build from source (see README)" ;;
esac

if [ -n "${KLAUB_VERSION:-}" ]; then
  version="${KLAUB_VERSION#v}"
else
  version="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" \
    | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$version" ] || fail "could not determine the latest version"
fi

archive="klaub-${version}-${target}.tar.gz"
base="https://github.com/${repo}/releases/download/v${version}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

printf 'Downloading klaub %s for %s\n' "$version" "$target"
curl -fsSL -o "$work/$archive" "$base/$archive"
curl -fsSL -o "$work/$archive.sha256" "$base/$archive.sha256"

expected="$(cut -d' ' -f1 < "$work/$archive.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$work/$archive" | cut -d' ' -f1)"
fi
[ "$expected" = "$actual" ] || fail "checksum mismatch for $archive"

tar -xzf "$work/$archive" -C "$work"
mkdir -p "$install_dir"
install -m 0755 "$work/klaub-${version}-${target}/klaub" "$install_dir/klaub"

printf 'Installed %s\n' "$install_dir/klaub"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to your PATH to run klaub.\n' "$install_dir" ;;
esac
"$install_dir/klaub" version
