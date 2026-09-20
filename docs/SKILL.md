---
name: stau
description: Manage dotfiles with stau, a GNU Stow-style symlink manager with per-package setup and teardown scripts. Use when installing, uninstalling, restowing, adopting, inspecting, or cleaning dotfile packages, or when the user mentions stau, STAU_DIR, or dotfile symlinks.
---

# stau

stau links the files of a package directory in a dotfiles repository (`STAU_DIR`) into a target directory (`STAU_TARGET`, default `$HOME`). It runs the package's `setup.sh` after linking on install and its `teardown.sh` before unlinking on uninstall. The README in the stau repository is the full reference; this skill covers operating stau safely.

## Configuration

- Dotfiles directory: `$STAU_DIR` if set, otherwise `~/.config/dotfiles`, otherwise `~/dotfiles`.
- Target directory: `-t, --target <dir>`, otherwise `$STAU_TARGET`, otherwise `$HOME`.

## Inspect before changing anything

- `stau list` shows every package as installed, partial, or not installed, with a count of broken links.
- `stau status <package>` shows each file as `[installed]`, `[not installed]`, `[conflict]`, `[restored]`, or `[BROKEN]`, and whether setup and teardown scripts exist.
- `stau status --json [<package>...]` prints the same state as a machine-readable envelope (one JSON object on stdout).
- `stau verify [--expect installed|uninstalled] <package>...` checks that every file is in the expected state and exits 0 on success, 5 on mismatch.
- Add `--dry-run` (`-n`) to any changing command first. It prints the symlinks, files, and scripts involved without touching the filesystem.

## Commands

| Task | Command |
|---|---|
| Link a package and run its setup script | `stau install <package>` |
| Link several packages in the order given | `stau install <package>...` |
| Link every package in `STAU_DIR` (alphabetical) | `stau install --all` |
| Link the ordered list in a profile | `stau install --profile <name>` |
| Remove the links and copy the files back | `stau uninstall <package>` |
| Relink after adding or moving files in a package (runs teardown, then setup) | `stau restow <package>` |
| Move existing files into a package and link them | `stau adopt <package> <file>...` |
| Remove broken links left by files deleted from a package | `stau clean <package>` |
| Check that every file of a package is in a given state | `stau verify --expect installed <package>` |

Flags: `--no-setup` and `--no-teardown` skip scripts, `-t, --target <dir>` overrides the target directory, and `-v, --verbose` explains each step. `install`, `uninstall`, and `restow` accept a package list, `--all`, or `--profile <name>`, and `--json` for a machine-readable summary. `--set KEY=VALUE` (repeatable) adds environment variables to hook scripts, `--non-interactive` connects their stdin to `/dev/null`, and `--script-timeout <dur>` (e.g. `30s`, `5m`) kills one that runs too long. `uninstall` also accepts `--all`, and `clean --deep` also finds links below directories removed from the package.

## Rules

- A package name is a single directory name inside `STAU_DIR`. Names such as `../x`, `a/b`, or absolute paths are rejected.
- An explicit package list is processed in the order given; `--all` is alphabetical (it has no dependency information); a profile preserves the order of its lines. Use a list or profile when hook order matters.
- Profiles live at `<STAU_DIR>/profiles/<name>`. Blank lines and `#` comments are ignored; every other line names one package. The `profiles` directory itself is never treated as a package.
- At the package root, `setup.sh`, `teardown.sh`, `.git`, `.gitignore`, `.gitattributes`, and `.gitmodules` are never linked. Files with these names in subdirectories are linked normally.
- `install` checks every target before creating any link. A conflicting file aborts it with exit code 2; remove the file or bring it under management with `stau adopt`. Never delete a user's file to resolve a conflict without asking.
- `adopt` takes regular files inside the target directory, as absolute or relative paths. Directories and symlinks are skipped with a warning, so adopt the files inside a directory individually. It creates the package directory if it is missing, and checks every file for conflicts before moving any.
- `uninstall` and `restow` remove only symlinks that point into the package; anything else at those paths is left alone.
- `uninstall` leaves standalone copies of the files in the target directory. Use `restow` to relink a package without copying files back.
- If `uninstall` cannot remove a link or copy a file back, it continues with the remaining files, reports every failure, and exits with the code of the last one. A failed copy-back restores the symlink so the file is not lost.
- Scripts must be executable. They run in the target directory with `STAU_DIR`, `STAU_PACKAGE`, and `STAU_TARGET` set, plus any `--set KEY=VALUE` variables, and inherit the terminal, so they may prompt for input. `--non-interactive` connects their stdin to `/dev/null` instead, and `--script-timeout <dur>` kills a script that exceeds it (a timeout is a script failure, exit 4). A failing setup script fails `install` and `restow`; a failing teardown script only warns, in both `uninstall` and `restow`.
- `clean` removes only broken symlinks that point into the package. `--deep` walks the whole target tree and can be slow for a large home directory.
- With `--all`, every package is processed even if some fail, and the command exits 1 if any did. `--json` on `install`, `uninstall`, and `restow` prints a JSON summary object on stdout instead of human text; hook scripts still inherit the terminal, so keep them quiet or redirect them when parsing stdout.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Package or `STAU_DIR` not found, invalid arguments, or a package failed during `--all` |
| 2 | Conflicting file |
| 3 | Permission denied or I/O error |
| 4 | Setup script failed (including a script timeout) |
| 5 | `verify` found a file not in the expected state |
