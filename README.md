# stau

A modern dotfile manager written in Rust that combines GNU Stow-style symlink management with powerful setup automation.

## Why stau?

Traditional tools like GNU Stow excel at symlink management but lack automation for the setup steps that often accompany dotfiles (installing dependencies, cloning repos, running configuration scripts). stau bridges this gap by providing both symlink management and scriptable setup hooks.

## Features

- **Symlink Management**: Stow-like symlinking from a dotfiles repository to your home or a custom directory
- **Setup Scripts**: Run package-specific setup/teardown scripts for automated configuration
- **Easy Adoption**: Migrate existing dotfiles into stau management with a single command
- **Written in Rust**: Fast, reliable, and cross-platform

## Quick Start

```bash
# Install a package (creates symlinks + runs setup script)
stau install <package> [--target <dir>]

# Adopt an existing dotfile into management
stau adopt <package> <file...> [--target <dir>]

# List managed packages
stau list [--target <dir>]

# Uninstall a package (removes symlinks, copies files back)
stau uninstall <package> [--target <dir>]

# Refresh symlinks for a package
stau restow <package> [--target <dir>]
```

## Project Structure

```
~/dotfiles/              # Your dotfiles repository
├── zsh/
│   ├── .zshrc
│   ├── .zshenv
│   ├── setup.sh         # Optional: runs on 'stau install zsh'
│   └── teardown.sh      # Optional: runs on 'stau uninstall zsh'
├── nvim/
│   └── .config/
│       └── nvim/
│           └── init.lua
└── git/
    └── .gitconfig
```

## Commands

**`stau install <package>`**
Creates symlinks from `~/dotfiles/<package>/` to your home directory and runs the package's `setup.sh` script if it exists.

**`stau uninstall <package>`**
Runs `teardown.sh` (if it exists), removes symlinks, and copies the actual files back to their original locations. This "unadopts" the dotfiles, leaving you with standalone config files.

**`stau adopt <package> <file...>`**
Moves existing files from your home directory into the dotfiles repository and replaces them with symlinks.

```bash
stau adopt zsh ~/.zshrc ~/.zshenv
# Moves files to ~/dotfiles/zsh/ and creates symlinks
```

**`stau list`**
Shows all managed packages and their status.

**`stau restow <package>`**
Removes and recreates symlinks for a package (useful after modifying the package structure).

## Setup Scripts

Each package can have optional scripts:

- **`setup.sh`**: Run during `stau install` for initial setup (install dependencies, clone repos, etc.)
- **`teardown.sh`**: Run during `stau uninstall` for cleanup (optional)

Example `~/dotfiles/zsh/setup.sh`:

```bash
#!/bin/bash
# Install oh-my-zsh
if [ ! -d "$STAU_TARGET/.oh-my-zsh" ]; then
    sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"
fi

# Clone zsh plugins
git clone https://github.com/zsh-users/zsh-autosuggestions "$STAU_TARGET/.oh-my-zsh/custom/plugins/zsh-autosuggestions"
```

Example `~/dotfiles/zsh/teardown.sh`:

```bash
#!/bin/bash
# Remove plugins installed during setup
rm -rf "$STAU_TARGET/.oh-my-zsh/custom/plugins/zsh-autosuggestions"
```

**Note**: Scripts receive these environment variables:

- `STAU_DIR`: Path to your dotfiles directory
- `STAU_PACKAGE`: Current package name
- `STAU_TARGET`: Where symlinks are created (use this instead of hardcoding `$HOME`)

## Configuration

### Dotfiles Directory

stau looks for your dotfiles directory at `~/dotfiles` by default. You can override this with the `STAU_DIR` environment variable:

```bash
export STAU_DIR="$HOME/.dotfiles"
```

### Target Directory

By default, stau creates symlinks in your home directory (`$HOME`). You can specify a different target directory using the `--target` flag or `STAU_TARGET` environment variable:

```bash
# Test installation without affecting your real home directory
stau install zsh --target /tmp/test-home

# Manage system configs
sudo stau install nginx --target /etc

# Use environment variable
export STAU_TARGET=/tmp/test
stau install zsh
stau list
```

This is useful for:

- **Testing**: Try out configurations in a temporary directory
- **Dry runs**: See what would happen without modifying your actual files
- **System configs**: Manage `/etc` or other system directories
- **Multiple users**: Install configs for different users

## Installation

```bash
cargo install stau
```

Or build from source:

```bash
git clone https://github.com/mhalder/stau
cd stau
cargo build --release
```

## License

[MIT](LICENSE)

## Contributing

Contributions welcome! Please open an issue or pull request.
