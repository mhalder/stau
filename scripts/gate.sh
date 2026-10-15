#!/usr/bin/env bash
# Every gate, in the order that fails fastest. A gate that lives only in a
# remote CI config is a gate discovered at the worst moment, so run this one.
#
# `./scripts/gate.sh [STAGE]` runs one stage; with no argument it runs every
# stage in the order below. CI runs one job per stage, so each stage checks
# only the tools it needs, while a bare run checks them all before any gate
# runs, as it always did.
set -euo pipefail
cd "$(dirname "$0")/.."

DEFAULT_COVERAGE_FLOOR=90
COVERAGE_FLOOR=${COVERAGE_FLOOR:-$DEFAULT_COVERAGE_FLOOR}
# The environment may raise the floor, never quietly lower it.
if ! [[ $COVERAGE_FLOOR =~ ^[0-9]+$ ]] || ((10#$COVERAGE_FLOOR < DEFAULT_COVERAGE_FLOOR)); then
  echo "COVERAGE_FLOOR must be a whole percent of at least ${DEFAULT_COVERAGE_FLOOR}, not '${COVERAGE_FLOOR}'" >&2
  exit 2
fi

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

STAGES="fmt clippy doc msrv test deny coverage"

stage=${1:-all}
case " $STAGES all " in
  *" $stage "*) ;;
  *)
    echo "unknown stage '$stage'; valid stages: ${STAGES// /, }, or all" >&2
    exit 2
    ;;
esac

# The tools a stage needs, collected here and refused together so a missing
# tool stops the run before any gate starts. `all` asks for every tool, as a
# bare run always did; a single stage asks only for its own.
missing=()
need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null || missing+=("$tool")
  done
}
refuse_missing() {
  if ((${#missing[@]})); then
    printf 'missing prerequisites: %s\n' "${missing[*]}" >&2
    echo "see CONTRIBUTING.md" >&2
    exit 2
  fi
}

prereq_fmt()      { need cargo; }
prereq_clippy()   { need cargo; }
prereq_doc()      { need cargo; }
prereq_msrv()     { need cargo rustup; }
prereq_test()     { need cargo; }
prereq_deny()     { need cargo cargo-deny; }
prereq_coverage() { need cargo cargo-llvm-cov; }

gate_fmt() {
  step "cargo fmt --check"
  cargo fmt --all -- --check
}

gate_clippy() {
  step "cargo clippy --all-targets -D warnings"
  cargo clippy --all-targets --all-features --locked -- -D warnings
}

gate_doc() {
  step "cargo doc --no-deps with rustdoc warnings denied"
  # A dangling doc link is only a warning rustdoc prints, and no other gate
  # builds the docs, so without this one it is found by hand or not at all.
  RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked
}

gate_msrv() {
  MSRV=$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' Cargo.toml)
  if [[ -z $MSRV ]]; then
    echo "Cargo.toml declares no rust-version" >&2
    exit 2
  fi
  step "cargo check --all-targets on the declared rust-version ${MSRV}"
  # Only a build on that toolchain shows the declared version still holds. A
  # separate target directory keeps it from invalidating the stable build.
  # Installing an installed toolchain still syncs its channel over the network, so
  # only a missing one is installed.
  if ! RUSTUP_AUTO_INSTALL=0 rustup run "$MSRV" rustc --version >/dev/null 2>&1; then
    rustup toolchain install "$MSRV" --profile minimal --no-self-update
  fi
  CARGO_TARGET_DIR=target/msrv cargo "+$MSRV" check --all-targets --all-features --locked
}

gate_test() {
  step "cargo test"
  cargo test --all-features --locked
}

gate_deny() {
  step "cargo deny check"
  # Advisories, yanked crates, licences and sources. It reads the same RustSec
  # database `cargo audit` would, so there is no separate audit step.
  cargo deny check
}

gate_coverage() {
  step "coverage (floor ${COVERAGE_FLOOR}% lines)"
  # A total floor only: several modules sit below the sibling projects' 95%
  # per-file bar, so a per-file floor here would need test work first.
  cargo llvm-cov --all-features --locked --summary-only \
    --fail-under-lines "$COVERAGE_FLOOR"
}

if [[ $stage == all ]]; then
  # A bare run refuses every missing tool before any gate runs.
  need cargo rustup cargo-deny cargo-llvm-cov
  refuse_missing
  for s in $STAGES; do
    "gate_$s"
  done
else
  "prereq_$stage"
  refuse_missing
  "gate_$stage"
fi

printf '\n\033[1;32mall gates green\033[0m\n'
