# Contributing

For what the tool is and how to use it, see the [README](README.md).

## Prerequisites

Beyond a stable Rust toolchain, a few tools live outside the repo. Each gate
stage checks only the tools it needs and names what is missing:

| Stage | Needs |
| --- | --- |
| `fmt`, `clippy`, `doc`, `test` | `cargo` |
| `msrv` | `cargo`, `rustup`; the gate installs the declared toolchain when it is missing |
| `deny` | `cargo install cargo-deny` |
| `coverage` | `cargo`, `cargo install cargo-llvm-cov` |

## The gate

One script runs every gate, in the order that fails fastest. Every gate must
pass before a commit:

```
./scripts/gate.sh
```

CI runs one job per stage, each `./scripts/gate.sh <stage>`. The stages are
`fmt`, `clippy`, `doc`, `msrv`, `test`, `deny`, `coverage`; no argument means
`all`, which runs them in that order. Each stage checks only its own
prerequisites and refuses a missing tool with exit 2 before running anything;
`all` checks every tool before any stage runs.

| Step | What it catches |
| --- | --- |
| `cargo fmt --check` | formatting |
| `cargo clippy --all-targets -D warnings` | a lint warning |
| `cargo doc --no-deps`, rustdoc warnings denied | a dangling doc link, which no other gate builds |
| `cargo check` on the declared `rust-version` | a dependency or language feature newer than the MSRV |
| `cargo test` | everything below |
| `cargo deny check` | a RustSec advisory, a yanked crate, a licence outside `deny.toml`'s list, an unknown source |
| coverage | a line floor of 90% in total |

The floor is a total only. The sibling projects apply the floor per file too,
but several stau modules sit below that bar, so a per-file floor here would
need test work first. `COVERAGE_FLOOR` may raise the floor, never lower it.

Every cargo build runs with `--locked`, so a manifest change the committed
`Cargo.lock` does not match fails the gate instead of being resolved quietly
mid-run.

The gate takes minutes; run it under `tmux-run` or a spare pane rather than in
a blocking call.

## Test discipline

`tests/integration_test.rs` drives the built binary end to end against a
temporary dotfiles directory and target, because the subject is the command
surface, its symlinks and rendered files, and its exit codes. A change begins
with a test that fails for the reason the requirement names; coverage is a
floor under that, never a substitute for it.

## Layout

```
src/main.rs         process setup and exit codes
src/cli.rs          the clap surface; the derive structs are the surface
src/commands/       one module per verb
src/config.rs       STAU_DIR and STAU_TARGET resolution
src/package.rs      package discovery and target mapping
src/symlink.rs      create, inspect and remove links and rendered files
src/template.rs     the minijinja/toml data model and rendering
src/script.rs       setup and teardown hooks
src/profile.rs      ordered package lists
scripts/gate.sh     every gate
```

## Adding a command

1. Add the variant to `src/cli.rs`.
2. Add a module under `src/commands/` and dispatch it from `src/commands/mod.rs`.
3. Test it in `tests/integration_test.rs` against a fixture directory.
4. Document it in `README.md` and, if the skill should route to it, in
   `skills/stau/SKILL.md`.

## Dependencies

`Cargo.lock` is committed, and every cargo build in the gate runs with
`--locked`. `cargo update` resolves only versions compatible with the declared
`rust-version`, and says which it held back. Raising the MSRV is a decision,
not a side effect of an update.

## Releasing

Releases follow [Semantic Versioning](https://semver.org/), driven by
conventional commits and [release-plz](https://release-plz.dev/). The version
lives in `Cargo.toml` and nowhere else.

The commit type decides the bump, and whether a release happens at all:

| Commit | Release |
| --- | --- |
| `feat:` | minor |
| `fix:` | patch |
| `perf:`, `refactor:` | patch, in "Changed" |
| `revert:` | patch, in "Removed" |
| any type with `!` | the breaking change, whatever the type |
| `build:`, `chore:`, `ci:`, `docs:`, `style:`, `test:` | no release, no changelog entry |

A breaking change is marked with `!` on the commit type; a `BREAKING CHANGE:`
footer alone on an otherwise skipped type (`docs:`, `chore:`, ...) does not
open a release PR.

Nobody bumps the version by hand. A merged commit of a releasing type makes
release-plz open or update a release PR that bumps `Cargo.toml` and
`Cargo.lock` and writes the `CHANGELOG.md` section. Merging that PR is the
release: release-plz creates the `vX.Y.Z` tag, the GitHub Release, and the
crates.io publish through Trusted Publishing, then dispatches `release.yml` to
build and attach the binaries. A deliberate bump that conventional commits
would not produce is an override applied to the release PR before merging, not
a hand edit to `Cargo.toml` on main.

The release PR is merged with the repository's admin bypass: a PR opened with
the default `GITHUB_TOKEN` runs no CI, so branch protection would otherwise
block the merge. The crates.io Trusted Publisher is registered for repository
`mhalder/stau`, workflow `release-plz.yml`, and environment `release`; the
first version, 3.1.1, was published before the move.

Do not edit `CHANGELOG.md` or the version yourself; the release PR owns both.

## License

By contributing, you agree that your contribution is dual-licensed under MIT OR Apache-2.0, unless you state otherwise.
