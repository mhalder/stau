# stau

[![CI](https://github.com/mhalder/stau/workflows/CI/badge.svg)](https://github.com/mhalder/stau/actions)
[![codecov](https://codecov.io/gh/mhalder/stau/branch/main/graph/badge.svg)](https://codecov.io/gh/mhalder/stau)
[![Crates.io](https://img.shields.io/crates/v/stau.svg)](https://crates.io/crates/stau)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A modern dotfile manager written in Rust that combines GNU Stow-style symlink management with powerful setup automation.

## Why stau?

Traditional tools like GNU Stow excel at symlink management but lack automation for the setup steps that often accompany dotfiles (installing dependencies, cloning repos, running configuration scripts). stau bridges this gap by providing both symlink management and scriptable setup hooks.

## Features

- **Symlink Management**: GNU Stow-style symlinking from a dotfiles repository to your home directory or custom target
- **Setup/Teardown Scripts**: Run package-specific scripts for automated configuration during install/uninstall
- **Easy Adoption**: Migrate existing dotfiles into stau management with a single command
- **Conflict Detection**: Safely detects and reports conflicts before overwriting files
- **Dry Run Mode**: Preview changes before making them with `--dry-run`
- **Broken Symlink Cleanup**: Clean up stale symlinks with the `clean` command
- **Command Aliases**: Short aliases for common commands (`i`, `u`, `s`, `l`, etc.)
- **XDG Compliant**: Supports `~/.config/dotfiles` (preferred) and `~/dotfiles` (fallback)
- **Linux Native**: Written in Rust for Linux with proper home directory and XDG detection

## Installation

### From crates.io

```bash
cargo install stau
```

### From source

```bash
git clone https://github.com/mhalder/stau
cd stau
cargo build --release
# Binary will be at target/release/stau
```

### Pre-built binaries

Download pre-built binaries for Linux x86_64 from the [releases page](https://github.com/mhalder/stau/releases).

## Quick Start

```bash
# Install a package (creates symlinks + runs setup script)
stau install ghostty

# List all packages and their status
stau list

# Show detailed status for a specific package
stau status ghostty

# Adopt existing dotfiles into stau management
stau adopt ghostty ~/.config/ghostty/config

# Uninstall a package (removes symlinks, copies files back)
stau uninstall ghostty

# Refresh symlinks for a package
stau restow ghostty

# Clean up broken symlinks
stau clean ghostty
```

## Project Structure

Organize your dotfiles in a directory structure where each subdirectory is a "package":

```
~/dotfiles/                    # Your dotfiles repository (STAU_DIR)
├── ghostty/
│   ├── .config/
│   │   └── ghostty/
│   │       └── config         # Symlinked to ~/.config/ghostty/config
│   ├── setup.sh               # Optional: runs on 'stau install ghostty'
│   └── teardown.sh            # Optional: runs on 'stau uninstall ghostty'
├── git/
│   └── .gitconfig             # Symlinked to ~/.gitconfig
└── tmux/
    └── .tmux.conf             # Symlinked to ~/.tmux.conf
```

## Commands

### `stau install <package>` (aliases: `i`, `add`)

Creates symlinks from your dotfiles package to the target directory and runs the package's `setup.sh` script if present.

```bash
stau install ghostty                    # Basic install
stau install ghostty --no-setup         # Skip setup script
stau install ghostty --target /tmp/test # Install to custom directory
stau install ghostty --dry-run          # Preview without making changes
stau install ghostty --verbose          # Show detailed output
```

**Options:**
| Flag | Description |
|------|-------------|
| `--no-setup` | Skip running the setup script |
| `-t, --target <DIR>` | Target directory (default: `$HOME` or `$STAU_TARGET`) |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

### `stau uninstall <package>` (aliases: `u`, `rm`)

Runs `teardown.sh` (if present), removes symlinks, and restores original files from the dotfiles repo. This "unadopts" the dotfiles, leaving you with standalone config files.

```bash
stau uninstall ghostty                  # Basic uninstall
stau uninstall ghostty --no-teardown    # Skip teardown script
stau uninstall ghostty --dry-run        # Preview without making changes
```

**Options:**
| Flag | Description |
|------|-------------|
| `--no-teardown` | Skip running the teardown script |
| `-t, --target <DIR>` | Target directory (default: `$HOME` or `$STAU_TARGET`) |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

**Note:** If the teardown script fails, uninstall continues anyway (with a warning).

### `stau restow <package>` (alias: `r`)

Removes and recreates symlinks for a package. Useful after modifying the package structure or adding new files. Unlike `uninstall`, this does **not** copy files back. By default, both teardown and setup scripts run during restow.

```bash
stau restow ghostty                     # Refresh symlinks (runs teardown + setup)
stau restow ghostty --no-setup          # Skip setup script
stau restow ghostty --no-teardown       # Skip teardown script
stau restow ghostty --dry-run           # Preview changes
```

**Options:**
| Flag | Description |
|------|-------------|
| `--no-setup` | Skip running the setup script |
| `--no-teardown` | Skip running the teardown script |
| `-t, --target <DIR>` | Target directory |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

### `stau adopt <package> <file...>` (alias: `a`)

Moves existing files from your home directory into the dotfiles repository and replaces them with symlinks. Creates the package directory if it doesn't exist.

```bash
stau adopt ghostty ~/.config/ghostty/config  # Adopt config file
stau adopt git ~/.gitconfig                  # Adopt single file
stau adopt tmux ~/.tmux.conf --dry-run       # Preview adoption
```

**Options:**
| Flag | Description |
|------|-------------|
| `-t, --target <DIR>` | Target directory |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

### `stau list` (aliases: `l`, `ls`)

Shows all packages in your dotfiles directory and their installation status.

```bash
stau list                           # List all packages
stau list --target /tmp/test        # Check status against custom target
```

**Output example:**

```
Packages in /home/user/dotfiles:

  ghostty              [installed]  1 symlink
  git                  [installed]  1 symlink
  tmux                 [not installed]
```

**Options:**
| Flag | Description |
|------|-------------|
| `-t, --target <DIR>` | Target directory to check status against |

### `stau status <package>` (alias: `s`)

Shows detailed status information for a specific package, including each file's symlink state.

```bash
stau status ghostty                     # Show detailed status
stau status git --target /tmp/test      # Check against custom target
```

**Output example:**

```
Status for package 'ghostty':

  Package directory: /home/user/dotfiles/ghostty
  Target directory:  /home/user
  Setup script:      /home/user/dotfiles/ghostty/setup.sh (exists)
  Teardown script:   (none)

Files (1 total):
  [installed]      /home/user/.config/ghostty/config

Summary: 1 installed, 0 not installed, 0 broken
```

**Options:**
| Flag | Description |
|------|-------------|
| `-t, --target <DIR>` | Target directory to check status against |

### `stau clean <package>` (alias: `c`)

Removes broken symlinks for a package. Useful when source files have been deleted or moved.

```bash
stau clean ghostty                      # Remove broken symlinks
stau clean ghostty --dry-run            # Preview what would be removed
stau clean ghostty --verbose            # Show each symlink being removed
```

**Options:**
| Flag | Description |
|------|-------------|
| `-t, --target <DIR>` | Target directory |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

## Setup and Teardown Scripts

Each package can have optional automation scripts:

- **`setup.sh`**: Runs during `stau install` for initial setup (install dependencies, clone repos, etc.)
- **`teardown.sh`**: Runs during `stau uninstall` for cleanup

### Environment Variables Available to Scripts

Scripts receive these environment variables:

| Variable       | Description                                                            |
| -------------- | ---------------------------------------------------------------------- |
| `STAU_DIR`     | Path to your dotfiles directory                                        |
| `STAU_PACKAGE` | Current package name                                                   |
| `STAU_TARGET`  | Target directory for symlinks (use this instead of hardcoding `$HOME`) |

### Example setup.sh

```bash
#!/bin/bash
# ~/dotfiles/ghostty/setup.sh

# Ensure ghostty config directory exists
mkdir -p "$STAU_TARGET/.config/ghostty"

# Set up any additional themes or resources
if [ ! -d "$STAU_TARGET/.config/ghostty/themes" ]; then
    echo "Downloading ghostty themes..."
    mkdir -p "$STAU_TARGET/.config/ghostty/themes"
fi

echo "Ghostty setup complete!"
```

### Example teardown.sh

```bash
#!/bin/bash
# ~/dotfiles/ghostty/teardown.sh

# Clean up any additional resources created during setup
rm -rf "$STAU_TARGET/.config/ghostty/themes"

echo "Ghostty teardown complete!"
```

**Important:** Make scripts executable with `chmod +x setup.sh teardown.sh`

## Configuration

### STAU_DIR (Dotfiles Directory)

stau looks for your dotfiles in the following locations (in order of priority):

1. `$STAU_DIR` environment variable (if set)
2. `~/.config/dotfiles` (XDG-compliant, preferred)
3. `~/dotfiles` (legacy fallback)

Override with the `STAU_DIR` environment variable:

```bash
export STAU_DIR="$HOME/.dotfiles"
```

### STAU_TARGET (Target Directory)

By default, stau creates symlinks in your home directory (`$HOME`). Override with `--target` flag or `STAU_TARGET` environment variable:

```bash
# Using environment variable
export STAU_TARGET="/tmp/test"
stau install ghostty

# Using --target flag (takes precedence)
stau install ghostty --target /tmp/test
```

**Use cases:**

- **Testing**: Try configurations in a temporary directory
- **Dry runs**: See what would happen without modifying real files
- **System configs**: Manage `/etc` or other system directories
- **Multiple users**: Install configs for different users

## Exit Codes

stau uses specific exit codes to indicate different error conditions:

| Code | Meaning                                                               |
| ---- | --------------------------------------------------------------------- |
| 0    | Success                                                               |
| 1    | Package not found, STAU_DIR not found, invalid path, or general error |
| 2    | Conflicting file exists                                               |
| 3    | Permission denied or I/O error                                        |
| 4    | Setup or teardown script failed                                       |

## Global Options

These options work with all commands:

| Flag        | Short | Description                                       |
| ----------- | ----- | ------------------------------------------------- |
| `--verbose` | `-v`  | Enable verbose output showing detailed operations |
| `--dry-run` | `-n`  | Show what would be done without making changes    |
| `--help`    | `-h`  | Show help information                             |
| `--version` | `-V`  | Show version information                          |

## Ignored Files

stau automatically ignores these files in package directories:

- `setup.sh` and `teardown.sh` (script files)
- `.git`, `.gitignore`, `.gitattributes`, `.gitmodules` (version control, at package root only)

## Tips and Best Practices

1. **Start with dry-run**: Use `--dry-run` to preview changes before making them
2. **Use verbose mode**: Add `--verbose` when troubleshooting
3. **Keep scripts idempotent**: Setup scripts should be safe to run multiple times
4. **Test in isolation**: Use `--target /tmp/test` to test packages safely
5. **Commit before changes**: If your dotfiles are in git, commit before running stau commands
6. **Document dependencies**: Note any system dependencies in your README or setup scripts

## Common Workflows

### Setting up a new machine

```bash
# Clone your dotfiles
git clone https://github.com/username/dotfiles ~/dotfiles

# Install packages
stau install ghostty
stau install git
stau install tmux
```

### Adding a new dotfile

```bash
# Option 1: Adopt existing file
stau adopt ghostty ~/.config/ghostty/config

# Option 2: Create in package directory, then restow
# (edit ~/dotfiles/ghostty/.config/ghostty/config)
stau restow ghostty
```

### Migrating from GNU Stow

stau uses the same directory structure as GNU Stow. Simply set `STAU_DIR` to your existing stow directory and start using stau commands. Add `setup.sh`/`teardown.sh` scripts as needed.

## Versioning

This project follows [Semantic Versioning](https://semver.org/):

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

See [CHANGELOG.md](CHANGELOG.md) for detailed release notes.

## License

[MIT](LICENSE)

## Contributing

Contributions welcome! Please open an issue or pull request on [GitHub](https://github.com/mhalder/stau).
