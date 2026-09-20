# stau

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
- **Ordered Package Lists**: Install several packages in one command, in the order given
- **Profiles**: Group packages into an ordered list with `--profile <name>`
- **Machine-readable Output**: `--json` summaries and a `verify` command with a stable exit code
- **Automation-safe Hooks**: `--non-interactive`, `--script-timeout`, and `--set` for hook scripts
- **XDG Compliant**: Supports `~/.config/dotfiles` (preferred) and `~/dotfiles` (fallback)
- **Linux Native**: Written in Rust for Linux with proper home directory and XDG detection

## Installation

### From source

```bash
git clone https://github.com/mhalder/stau.git
cd stau
cargo install --path .
```

## Quick Start

```bash
# Install a package (creates symlinks + runs setup script)
stau install ghostty

# Install several packages in the order given
stau install fish tmux git

# Install all packages at once (alphabetical order)
stau install --all

# Install the ordered list in a profile
stau install --profile headless

# List all packages and their status
stau list

# Show detailed status for a specific package
stau status ghostty

# Show a machine-readable status envelope
stau status --json

# Verify every file of a package is installed
stau verify --expect installed ghostty

# Adopt existing dotfiles into stau management
stau adopt ghostty ~/.config/ghostty/config

# Uninstall a package (removes symlinks, copies files back)
stau uninstall ghostty

# Uninstall all packages at once
stau uninstall --all

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

### `stau install [package...]` (aliases: `i`, `add`)

Creates symlinks from your dotfiles packages to the target directory and runs each package's `setup.sh` script if present. Multiple package names are installed in the order given; `--all` installs every package alphabetically; `--profile <name>` installs the ordered list from a profile.

```bash
stau install ghostty                    # Basic install
stau install fish tmux git              # Several packages, in this order
stau install --all                      # Install all packages (alphabetical)
stau install --profile headless         # Install a profile's ordered list
stau install ghostty --no-setup         # Skip setup script
stau install ghostty --target /tmp/test # Install to custom directory
stau install ghostty --dry-run          # Preview without making changes
stau install ghostty --verbose          # Show detailed output
stau install --all --json               # Machine-readable summary
stau install fish --set DOTFILES_GUI=1 --non-interactive
```

**Options:**
| Flag | Description |
|------|-------------|
| `-a, --all` | Install all packages in STAU_DIR (alphabetical order) |
| `-p, --profile <NAME>` | Install the packages listed in `STAU_DIR/profiles/<NAME>` |
| `--no-setup` | Skip running the setup script |
| `--json` | Print a machine-readable JSON summary on stdout |
| `--set KEY=VALUE` | Set an environment variable for hook scripts (repeatable) |
| `--non-interactive` | Connect hook script stdin to `/dev/null` |
| `--script-timeout <DUR>` | Kill a hook script that runs longer than this (e.g. `30s`, `5m`) |
| `-t, --target <DIR>` | Target directory (default: `$HOME` or `$STAU_TARGET`) |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

### `stau uninstall [package...]` (aliases: `u`, `rm`)

Runs `teardown.sh` (if present), removes symlinks, and restores original files from the dotfiles repo. This "unadopts" the dotfiles, leaving you with standalone config files. Multiple package names are processed in the order given; `--all` processes every package alphabetically.

```bash
stau uninstall ghostty                  # Basic uninstall
stau uninstall fish tmux git            # Several packages, in this order
stau uninstall --all                    # Uninstall all packages
stau uninstall ghostty --no-teardown    # Skip teardown script
stau uninstall ghostty --dry-run        # Preview without making changes
stau uninstall --all --json             # Machine-readable summary
```

**Options:**
| Flag | Description |
|------|-------------|
| `-a, --all` | Uninstall all packages in STAU_DIR (alphabetical order) |
| `-p, --profile <NAME>` | Uninstall the packages listed in `STAU_DIR/profiles/<NAME>` |
| `--no-teardown` | Skip running the teardown script |
| `--json` | Print a machine-readable JSON summary on stdout |
| `--set KEY=VALUE` | Set an environment variable for hook scripts (repeatable) |
| `--non-interactive` | Connect hook script stdin to `/dev/null` |
| `--script-timeout <DUR>` | Kill a hook script that runs longer than this (e.g. `30s`, `5m`) |
| `-t, --target <DIR>` | Target directory (default: `$HOME` or `$STAU_TARGET`) |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

**Note:** If the teardown script fails, uninstall continues anyway (with a warning).

If a symlink cannot be removed or its file cannot be copied back, uninstall still processes the remaining files, reports every failure, and exits with the error code of the last one. When copying a file back fails, its symlink is restored so the target file is not left missing.

### `stau restow <package...>` (alias: `r`)

Removes and recreates symlinks for one or more packages. Useful after modifying the package structure or adding new files. Unlike `uninstall`, this does **not** copy files back. By default, both teardown and setup scripts run during restow. A package list is processed in the order given; `--all` restows every package alphabetically; `--profile <name>` restows a profile's ordered list.

```bash
stau restow ghostty                     # Refresh symlinks (runs teardown + setup)
stau restow ghostty --no-setup          # Skip setup script
stau restow ghostty --no-teardown       # Skip teardown script
stau restow --all                       # Restow every package
stau restow --all --json                # Machine-readable summary
stau restow ghostty --dry-run           # Preview changes
```

**Options:**
| Flag | Description |
|------|-------------|
| `-a, --all` | Restow all packages in STAU_DIR (alphabetical order) |
| `-p, --profile <NAME>` | Restow the packages listed in `STAU_DIR/profiles/<NAME>` |
| `--no-setup` | Skip running the setup script |
| `--no-teardown` | Skip running the teardown script |
| `--json` | Print a machine-readable JSON summary on stdout |
| `--set KEY=VALUE` | Set an environment variable for hook scripts (repeatable) |
| `--non-interactive` | Connect hook script stdin to `/dev/null` |
| `--script-timeout <DUR>` | Kill a hook script that runs longer than this (e.g. `30s`, `5m`) |
| `-t, --target <DIR>` | Target directory |
| `-n, --dry-run` | Show what would be done without making changes |
| `-v, --verbose` | Show detailed output |

### `stau adopt <package> <file...>` (alias: `a`)

Moves existing files from your home directory into the dotfiles repository and replaces them with symlinks. Creates the package directory if it doesn't exist.

Paths may be absolute or relative to the current directory, and must be inside the target directory. Only regular files can be adopted: directories and symlinks are skipped with a warning, so adopt the files inside a directory individually. All files are checked for conflicts before any file is moved.

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

### `stau status <package...>` (alias: `s`)

Shows detailed status information for a specific package, including each file's symlink state. `--json` prints a machine-readable envelope for one or more packages (or every package when none are named) and accepts `[restored]` as a file state — a regular file whose contents equal the package source, which is what `uninstall` leaves behind.

```bash
stau status ghostty                     # Show detailed status
stau status git --target /tmp/test      # Check against custom target
stau status --json                      # Envelope for every package
stau status --json ghostty git          # Envelope for named packages
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
| `--json` | Print a machine-readable JSON status envelope |
| `-t, --target <DIR>` | Target directory to check status against |

### `stau verify <package...>` (alias: `v`)

Checks that every file of every named package is in the expected state and exits non-zero on the first mismatch. `--expect installed` (the default) requires each file to be a symlink into the package; `--expect uninstalled` requires each file to be a `restored` plain copy whose contents match the package source.

```bash
stau verify --expect installed ghostty   # Exit 0 only if fully linked
stau verify --expect uninstalled ghostty # Exit 0 only if fully copied back
stau verify --json ghostty               # Machine-readable mismatches
```

**Options:**
| Flag | Description |
|------|-------------|
| `--expect installed\|uninstalled` | The state every file must be in (default `installed`) |
| `--json` | Print mismatches as a machine-readable JSON object |
| `-t, --target <DIR>` | Target directory to check against |

Without `--json`, one line per mismatch is printed. `verify` exits 0 on success, 5 on mismatch, 1 when a named package does not exist, and 3 on I/O errors.

### `stau clean <package>` (alias: `c`)

Removes broken symlinks that point into a package, such as links left behind after files were deleted from or moved within the package. Broken symlinks that point elsewhere are never removed.

By default, only target directories that mirror a directory still present in the package are scanned, which is fast. Links below a directory removed from the package entirely are found only with `--deep`, which walks the whole target directory tree without following symlinked directories, and can be slow for a large home directory.

```bash
stau clean ghostty                      # Remove broken symlinks
stau clean ghostty --deep               # Also find links below removed directories
stau clean ghostty --dry-run            # Preview what would be removed
stau clean ghostty --verbose            # Show each symlink being removed
```

**Options:**
| Flag | Description |
|------|-------------|
| `--deep` | Scan the whole target directory tree |
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

Scripts inherit the terminal, so they may prompt for input and their output is shown as it is produced. For automation:

- `--set KEY=VALUE` (repeatable) adds environment variables to the script process.
- `--non-interactive` connects the script's stdin to `/dev/null`, so a prompting hook fails immediately instead of blocking.
- `--script-timeout <DUR>` kills a script that runs longer than the given duration (e.g. `30s`, `5m`, `500ms`). A timeout is reported as a script failure (exit code 4).

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

## Profiles

A profile is an ordered package list stored as a plain text file at `<STAU_DIR>/profiles/<name>`. Blank lines and lines starting with `#` are ignored; every other line names one package. The order of the lines is the order packages are processed, which is what lets a profile express dependency order for hooks (e.g. `mise` before `sesh`).

```bash
stau install --profile headless
stau uninstall --profile headless
stau restow --profile headless
```

Example `profiles/headless`:

```
# base tooling first
mise
ripgrep
sesh
```

The `profiles` directory itself is never treated as a package by `--all`, `list`, or `status --json`.

## Machine-readable Output

Three commands produce stable machine-readable output:

- `stau status --json [<package>...]` prints one JSON object with `stau_dir`, `target`, and a `packages` array of per-package state, counts, and files.
- `stau verify [--json] <package>...` prints per-file mismatches; with `--json` it prints `{"ok": bool, "packages": [...]}` and exits 5 on any mismatch.
- `stau install|uninstall|restow --json` print a summary object `{"ok": bool, "packages": [{"name", "ok", "error"}]}` on stdout, also when some packages failed (the command still exits 1).

In `--json` mode stau suppresses its own human output, but hook scripts still inherit the terminal; keep scripts quiet or redirect their output when parsing stdout.

## Exit Codes

stau uses specific exit codes to indicate different error conditions:

| Code | Meaning                                                               |
| ---- | --------------------------------------------------------------------- |
| 0    | Success                                                               |
| 1    | Package not found, STAU_DIR not found, invalid path, or general error |
| 2    | Conflicting file exists                                               |
| 3    | Permission denied or I/O error                                        |
| 4    | Setup script failed (including a script timeout)                      |
| 5    | `verify` found a file not in the expected state                       |

With `--all`, every package is processed even if some fail. The command then exits with code 1 if any package failed.

## Global Options

These options work with all commands:

| Flag        | Short | Description                                       |
| ----------- | ----- | ------------------------------------------------- |
| `--verbose` | `-v`  | Enable verbose output showing detailed operations |
| `--dry-run` | `-n`  | Show what would be done without making changes    |
| `--help`    | `-h`  | Show help information                             |
| `--version` | `-V`  | Show version information                          |

## Ignored Files

stau automatically ignores these files at the package root:

- `setup.sh` and `teardown.sh` (script files)
- `.git`, `.gitignore`, `.gitattributes`, `.gitmodules` (version control)

Files with these names in subdirectories (for example `.local/bin/setup.sh`) are linked normally.

## Agent Skill

[`docs/SKILL.md`](docs/SKILL.md) is a [pi](https://pi.dev) skill that teaches a coding agent to operate stau safely: inspecting packages before changing them, previewing with `--dry-run`, resolving conflicts, and interpreting exit codes. Load it by path, for example:

```bash
pi --skill docs/SKILL.md
```

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

# Install all packages at once
stau install --all

# Or install individual packages
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

Private project. Contributions handled internally.
