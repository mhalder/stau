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
