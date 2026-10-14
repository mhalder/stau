# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### ⚠ BREAKING CHANGES

* **list:** the human-readable output changed from bracketed labels (`[installed]`, `[not installed]`, `[partial]`, `[hook only]`) to a per-package state token (`ok`, `partial`, `missing`, `conflict`, `hook`) with a notes column. Scripts that parse `stau list` must be updated; `stau status --json` remains the machine-readable interface.

### Features

* **templating:** render `*.tmpl` sources to regular files at their target with minijinja, using per-machine data from `stau.*` facts, `<STAU_DIR>/.stau/data.toml`, `~/.config/stau/machine.toml`, and `--set KEY=VALUE` (deep-merged; missing variables are strict errors)
* **cli:** add `stau data` (print the merged data as TOML or `--json`; `stau data set <key> <value>` upserts into `machine.toml`) and `stau render <file>` (render one template to stdout)
* **cli:** add `adopt --template`, storing the adopted file as `<rel>.tmpl` and rendering it back to the target
* **status:** add the `drift` file state (a rendered target whose bytes differ from the current render) and the `error` file state (a template that failed to render) to `status`/`list` output, the JSON envelope, and `verify` mismatches
* **list:** report the new package state token (`ok`, `partial`, `missing`, `conflict`, `hook`) with a notes column, and add `--long` (`-l`) to show link counts and hooks for every package
* **cli:** add `stau completion <shell>`, printing a static completion script for bash, elvish, fish, powershell, or zsh; it runs without a `STAU_DIR`
* **cli:** hint `--target` as a directory and `adopt` file arguments as paths, so shells complete paths for them

### Bug Fixes

* **cli:** restore the default `SIGPIPE` disposition, so piping long output into a command that exits early (`stau status pkg | head`) terminates the way any Unix command does instead of panicking with exit code 101

## [3.1.1](https://github.com/mhalder/stau/compare/v3.1.0...v3.1.1) (2026-03-16)

### Bug Fixes

* **test:** use CARGO_BIN_EXE for binary lookup instead of path manipulation ([6511e8c](https://github.com/mhalder/stau/commit/6511e8ca247462371bff1d2ae2417d40f7283f94)), closes [#20](https://github.com/mhalder/stau/issues/20)

## [3.1.0](https://github.com/mhalder/stau/compare/v3.0.0...v3.1.0) (2026-01-20)

### Features

* **cli:** add --all flag to install command ([58c3b15](https://github.com/mhalder/stau/commit/58c3b15c2e67b7759e3ce18a55e41b37d485e910))
* **cli:** add --all flag to uninstall command ([61d6df4](https://github.com/mhalder/stau/commit/61d6df490abf9d9e14bf7ecc2a3f606d8ae8a189))

## [3.0.0](https://github.com/mhalder/stau/compare/v2.0.2...v3.0.0) (2026-01-19)

### ⚠ BREAKING CHANGES

* **cli:** restow now runs both teardown and setup scripts by
default. Use --no-setup and/or --no-teardown to skip scripts.

Migration:
- Old: stau restow pkg → New: stau restow pkg --no-setup --no-teardown
- Old: stau restow pkg --run-setup → New: stau restow pkg

### Bug Fixes

* **cli:** standardize restow flags to match install/uninstall pattern ([078bd86](https://github.com/mhalder/stau/commit/078bd86f6dca506fb09b76512495c31269f52833))

## [2.0.2](https://github.com/mhalder/stau/compare/v2.0.1...v2.0.2) (2026-01-19)

### Bug Fixes

* restore Rust 2024 edition and modernize with let chains ([12342ae](https://github.com/mhalder/stau/commit/12342ae73d3281a5f9a0ad76d5bcb52c608b5594)), closes [#14](https://github.com/mhalder/stau/issues/14) [#16](https://github.com/mhalder/stau/issues/16)

## [2.0.1](https://github.com/mhalder/stau/compare/v2.0.0...v2.0.1) (2026-01-18)

### Bug Fixes

* correct Rust edition and clean up codebase ([90afa52](https://github.com/mhalder/stau/commit/90afa52833f17bc51186f09df291094a4176afaf))

### Documentation

* replace zsh examples with ghostty ([#12](https://github.com/mhalder/stau/issues/12)) ([3a8a9f8](https://github.com/mhalder/stau/commit/3a8a9f89fc5bf00f472add7d08affe79f9611945)), closes [#11](https://github.com/mhalder/stau/issues/11)

## [2.0.0](https://github.com/mhalder/stau/compare/v1.1.0...v2.0.0) (2025-12-29)

### ⚠ BREAKING CHANGES

* Remove --force/-f flags from install and uninstall commands.

Users should now either remove conflicting files manually or use
'stau adopt' to bring existing files under stau management.

- Remove --force/-f from install command
- Remove --force from uninstall command
- Simplify create_symlink to error on conflicts
- Update error messages to remove --force suggestion
- Update README documentation
- Remove force-related tests

### Features

* remove --force command line flags ([#10](https://github.com/mhalder/stau/issues/10)) ([5d46b72](https://github.com/mhalder/stau/commit/5d46b729d992a43dc1702238cd89eaef584de144)), closes [#9](https://github.com/mhalder/stau/issues/9)

## [1.1.0](https://github.com/mhalder/stau/compare/v1.0.2...v1.1.0) (2025-12-29)

### Features

* add command aliases, XDG compliance, and cross-platform support ([#8](https://github.com/mhalder/stau/issues/8)) ([d021670](https://github.com/mhalder/stau/commit/d02167020d551468a2c1c4bc21d7d759c3d6ca9b)), closes [#5](https://github.com/mhalder/stau/issues/5)

## [1.0.2](https://github.com/mhalder/stau/compare/v1.0.1...v1.0.2) (2025-10-15)

### Bug Fixes

* resolve uninstall --dry-run conflict error and add comprehensive option coverage ([752c268](https://github.com/mhalder/stau/commit/752c268e319b97a2ade0b784174c44efadf07ca8))

## [1.0.1](https://github.com/mhalder/stau/compare/v1.0.0...v1.0.1) (2025-10-14)

### Bug Fixes

* correct symlink metadata handling in --force flag implementation ([2fd0a70](https://github.com/mhalder/stau/commit/2fd0a70a84294b39c8e63c5d8c9a88a64746287e))

## 1.0.0 (2025-10-14)

### Features

* add exit codes, --force flag, integration tests, and enhanced error messages ([a8ac14f](https://github.com/mhalder/stau/commit/a8ac14fc3ee53777bc2aed1a77a1b1304d577cb2))
* implement core install and uninstall commands ([803dd8a](https://github.com/mhalder/stau/commit/803dd8a420dce7931ab3dc1f57988e585d34f33e))
* implement foundational CLI structure and configuration ([e0c173a](https://github.com/mhalder/stau/commit/e0c173a228b1c9eaf18ae8c7c0741d0dbf44b435))
* implement remaining commands (list, restow, adopt, status, clean) ([0662aee](https://github.com/mhalder/stau/commit/0662aee797c6207601d9a49e3eaab4e32e0d48ab))

### Bug Fixes

* correct cargo registry token env var syntax in release config ([ad62488](https://github.com/mhalder/stau/commit/ad62488a7829ec288e47c736402b8df79d289afd))

### Documentation

* add semantic versioning and changelog ([2a501c4](https://github.com/mhalder/stau/commit/2a501c467b997afc1634d11e1b68ceb9deff175e))
