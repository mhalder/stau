# Security

## Supported versions

| Version | Supported |
| ------- | --------- |
| 3.1.x   | Yes       |

## Reporting a vulnerability

Report vulnerabilities through GitHub private vulnerability reporting:

https://github.com/mhalder/stau/security/advisories/new

Do not open a public issue for a security report.

## What matters here

stau links a dotfiles repository into a target directory and runs its hooks, so
the security-relevant surface is:

- **Hook execution.** `setup.sh` and `teardown.sh` run with the user's
  privileges and inherited stdio. A dotfiles repository is trusted input; a
  package from an untrusted source can run arbitrary code on install.
  `--non-interactive`, `--script-timeout`, and `--dry-run` limit exposure but
  are not a sandbox.
- **Path handling.** Package names and file paths under `STAU_DIR` are resolved
  against the target directory, and a name that escapes the dotfiles root is
  rejected. A symlink inside a package is followed when writing.
- **Template rendering.** Rendered `*.tmpl` files read per-machine data from
  `stau.*` facts, `<STAU_DIR>/.stau/data.toml`, `~/.config/stau/machine.toml`,
  and `--set`. Rendering writes regular files over their targets; keep the data
  files out of untrusted hands.
- **No network at runtime.** The binary never talks to the network.
