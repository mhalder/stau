#!/bin/sh
# Check that the crate version has a single source of truth.
#
# Usage: scripts/check-version.sh [EXPECTED]
#
# EXPECTED is the tag being released (v0.1.0) or a bare version. When it is
# given, Cargo.toml must declare exactly that version. release.yml passes the
# pushed tag, so a tag that disagrees with Cargo.toml stops the release.
#
# Always checked:
#   - Cargo.toml declares a valid semantic version, without build metadata
#   - CHANGELOG.md has a dated section for that version
#
# Exits 0 when everything agrees, 1 otherwise.
set -eu

fail() {
  echo "error: $1" >&2
  exit 1
}

cd "$(dirname "$0")/.."

expected=${1-}
case $expected in
  v*) expected=${expected#v} ;;
esac

# Read the [package] version without needing cargo.
version=$(awk '
  /^\[/ { in_package = ($0 == "[package]") }
  in_package && /^version[[:space:]]*=/ {
    sub(/#.*$/, "")
    sub(/^[^=]*=[[:space:]]*/, "")
    gsub(/[[:space:]"]/, "")
    print
    exit
  }
' Cargo.toml)

[ -n "$version" ] || fail "Cargo.toml has no version in its [package] section"

case $version in
  *+*) fail "version $version carries build metadata, which crates.io rejects" ;;
esac

printf '%s\n' "$version" |
  grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$' ||
  fail "version $version is not a valid semantic version"

if [ -n "$expected" ] && [ "$version" != "$expected" ]; then
  fail "the release tag names $expected but Cargo.toml declares $version"
fi

grep -Fq "## [$version]" CHANGELOG.md ||
  fail "CHANGELOG.md has no '## [$version]' section"

# release-plz writes "## [1.2.3] - 2026-10-07" for the first release and
# "## [1.2.3](compare-url) - 2026-10-07" when there is a previous release.
awk -v v="$version" '
  index($0, "## [" v "]") == 1 {
    rest = substr($0, length(v) + 6)
    sub(/^\([^)]*\)/, "", rest)
    if (rest ~ /^ - [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) found = 1
  }
  END { exit(found ? 0 : 1) }
' CHANGELOG.md ||
  fail "CHANGELOG.md needs a dated heading: '## [$version] - YYYY-MM-DD'"

echo "version $version agrees with Cargo.toml and CHANGELOG.md"
