---
name: stau
description: Manage dotfiles with stau, a GNU Stow-style symlink manager with per-package setup and teardown scripts. Use when installing, uninstalling, restowing, adopting, inspecting, or cleaning dotfile packages, or when the user mentions stau, STAU_DIR, or dotfile symlinks.
---

# stau

stau links the files of a package directory in a dotfiles repository (`STAU_DIR`) into a target directory (`STAU_TARGET`, default `$HOME`). It runs the package's `setup.sh` after linking on install and its `teardown.sh` before unlinking on uninstall. The [README](../README.md) is the full reference; this skill covers operating stau safely.

## Inspect before changing anything

- `stau list` shows every package as installed, partial, or not installed, with a count of broken links.
- `stau status <package>` shows each file as `[installed]`, `[not installed]`, `[conflict]`, or `[BROKEN]`, and whether setup and teardown scripts exist.
- Add `--dry-run` (`-n`) to any changing command first. It prints the symlinks, files, and scripts involved without touching the filesystem.

## Commands

| Task | Command |
|---|---|
| Link a package and run its setup script | `stau install <package>` |
| Link every package in `STAU_DIR` | `stau install --all` |
| Remove the links and copy the files back | `stau uninstall <package>` |
| Relink after adding or moving files in a package | `stau restow <package>` |
| Move existing files into a package and link them | `stau adopt <package> <file>...` |
| Remove broken links left by files deleted from a package | `stau clean <package>` |

Flags: `--no-setup` and `--no-teardown` skip scripts, `-t, --target <dir>` overrides the target directory, and `-v, --verbose` explains each step. `uninstall` also accepts `--all`, and `clean --deep` also finds links below directories removed from the package.

## Rules

- A package name is a single directory name inside `STAU_DIR`. Names such as `../x`, `a/b`, or absolute paths are rejected.
- `install` checks every target before creating any link. A conflicting file aborts it with exit code 2; remove the file or bring it under management with `stau adopt`. Never delete a user's file to resolve a conflict without asking.
- `adopt` takes regular files inside the target directory, as absolute or relative paths. Directories and symlinks are skipped with a warning, so adopt the files inside a directory individually.
- `uninstall` leaves standalone copies of the files in the target directory. Use `restow` to relink a package without copying files back.
- Scripts must be executable. They run in the target directory with `STAU_DIR`, `STAU_PACKAGE`, and `STAU_TARGET` set, and inherit the terminal, so they may prompt for input. A failing setup script fails `install`; a failing teardown script only warns.
- `clean` removes only broken symlinks that point into the package. `--deep` walks the whole target tree and can be slow for a large home directory.
- With `--all`, every package is processed even if some fail, and the command exits 1 if any did.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Package or `STAU_DIR` not found, invalid arguments, or a package failed during `--all` |
| 2 | Conflicting file |
| 3 | Permission denied or I/O error |
| 4 | Setup script failed |
