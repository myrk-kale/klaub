#!/usr/bin/env bash
#
# Prints the CHANGELOG.md section of one version (the text under "## [X.Y.Z]" up to the
# next "## [" heading), for use as GitHub release notes.
#
# Usage: scripts/release-notes.sh 0.1.0

set -euo pipefail

version="${1:?usage: release-notes.sh <version>}"
changelog="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/CHANGELOG.md"

notes="$(awk -v v="${version}" '
  $0 ~ "^## \\[" v "\\]" { capture = 1; next }
  capture && /^## \[/    { exit }
  capture                { print }
' "${changelog}")"

if [ -z "$(printf '%s' "${notes}" | tr -d '[:space:]')" ]; then
  echo "no changelog entry found for version ${version}" >&2
  exit 1
fi
printf '%s\n' "${notes}"
