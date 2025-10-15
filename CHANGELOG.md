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

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Correct symlink metadata handling in --force flag implementation
  - Fixed incorrect use of `is_file()` and `is_dir()` which follow symlinks
  - Now uses `symlink_metadata()` consistently to check actual file types
  - Fixed uninstall --force to properly handle directories with `remove_dir_all`

## [0.1.0] - 2024-01-XX

### Added

- Initial release of stau
- Core symlink management commands (install, uninstall, restow)
- Package adoption with `adopt` command
- Package listing with `list` command
- Detailed status reporting with `status` command
- Broken symlink cleanup with `clean` command
- Setup and teardown script support
- Dry-run mode with `--dry-run` flag
- Force installation/uninstallation with `--force` flag
- Configurable target directory via `--target` flag or `STAU_TARGET` env var
- Configurable dotfiles directory via `STAU_DIR` env var
- Exit codes for different error types
- Comprehensive error messages with helpful hints
- Integration and unit test suite
- CI/CD workflow for automated testing and linting

### Changed

- N/A

### Deprecated

- N/A

### Removed

- N/A

### Fixed

- N/A

### Security

- N/A

[Unreleased]: https://github.com/mhalder/stau/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mhalder/stau/releases/tag/v0.1.0
