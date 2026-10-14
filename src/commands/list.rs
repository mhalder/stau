use crate::config::Config;
use crate::error::Result;
use crate::package;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::status::{FileState, compute_file_state_lenient};

/// Lists all packages in STAU_DIR and their installation status.
///
/// Each package is one row: a short state token (`ok`, `partial`, `missing`,
/// `conflict`, `hook`) and, only when it adds information, a note column with
/// link counts, available hooks, and broken links. `long` prints the link count
/// for every row instead of only the ones needing attention.
pub fn list_packages(
    config: &Config,
    target: Option<PathBuf>,
    long: bool,
    data: &serde_json::Value,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let packages = package::list_packages(&config.stau_dir)?;

    if packages.is_empty() {
        println!("No packages found in {}", config.stau_dir.display());
        return Ok(());
    }

    let rows: Vec<(&str, std::result::Result<PackageStatus, String>)> = packages
        .iter()
        .map(|pkg| {
            (
                pkg.as_str(),
                PackageStatus::collect(config, pkg, &target_dir, data).map_err(|e| e.to_string()),
            )
        })
        .collect();

    let name_width = rows.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    let state_width = rows
        .iter()
        .map(|(_, result)| match result {
            Ok(status) => status.state.len(),
            Err(_) => "error".len(),
        })
        .max()
        .unwrap_or(0);

    println!("Packages in {}", config.stau_dir.display());
    println!("Target: {}", target_dir.display());
    println!();

    for (name, result) in &rows {
        match result {
            Ok(status) => print_row(
                name,
                status.state,
                &status.notes(long),
                name_width,
                state_width,
            ),
            Err(message) => print_row(name, "error", message, name_width, state_width),
        }
    }

    print_summary(&rows);
    Ok(())
}

/// Prints one aligned package row, trimming the padding when there is no note.
fn print_row(name: &str, state: &str, notes: &str, name_width: usize, state_width: usize) {
    let mut line = format!("  {name:<name_width$}  {state:<state_width$}");
    if !notes.is_empty() {
        line.push_str("  ");
        line.push_str(notes);
    }
    println!("{}", line.trim_end());
}

/// What `list` can say about one package. `broken` is orthogonal to `state`: a
/// package can be fully linked and still own stale links to removed files.
struct PackageStatus {
    state: &'static str,
    installed: usize,
    total: usize,
    broken: usize,
    drift: usize,
    has_setup: bool,
    has_teardown: bool,
}

impl PackageStatus {
    fn collect(
        config: &Config,
        pkg: &str,
        target_dir: &Path,
        data: &serde_json::Value,
    ) -> Result<Self> {
        let package_dir = config.get_package_dir(pkg);
        let mappings = package::discover_package_files(&package_dir, target_dir, data)?;
        let broken = package::discover_broken_symlinks(&package_dir, target_dir)?;
        let broken_set: HashSet<PathBuf> = broken.iter().cloned().collect();

        let mut installed = 0;
        let mut conflict = 0;
        let mut drift = 0;
        let mut broken_in_mappings = 0;
        for mapping in &mappings {
            match compute_file_state_lenient(mapping, &broken_set) {
                FileState::Installed => installed += 1,
                FileState::Conflict => conflict += 1,
                FileState::Broken => broken_in_mappings += 1,
                FileState::Drift => drift += 1,
                FileState::NotInstalled | FileState::Restored => {}
            }
        }

        // Broken symlinks whose file was removed from the package entirely are
        // not in `mappings` but still belong to the package.
        let stale_broken = broken
            .iter()
            .filter(|path| !mappings.iter().any(|mapping| mapping.target == **path))
            .count();

        let has_setup = config.get_setup_script(pkg).is_some();
        let has_teardown = config.get_teardown_script(pkg).is_some();
        let total = mappings.len();

        // Mirrors the package state in `status`; `hook` is a package shape, not
        // an observed installation, because stau does not record hook runs.
        let state = if conflict > 0 {
            "conflict"
        } else if total == 0 && (has_setup || has_teardown) {
            "hook"
        } else if total == 0 || installed == 0 {
            "missing"
        } else if installed == total {
            "ok"
        } else {
            "partial"
        };

        Ok(Self {
            state,
            installed,
            total,
            broken: broken_in_mappings + stale_broken,
            drift,
            has_setup,
            has_teardown,
        })
    }

    fn notes(&self, long: bool) -> String {
        let mut parts: Vec<String> = Vec::new();

        match self.state {
            "ok" if long => parts.push(link_phrase(self.installed)),
            "partial" | "conflict" => {
                parts.push(format!("{}/{} links", self.installed, self.total))
            }
            "missing" if self.total > 0 => parts.push(format!("0/{} links", self.total)),
            _ => {}
        }

        if self.drift > 0 {
            parts.push(format!("{} drift", self.drift));
        }

        // Hooks are stau's differentiator, but most packages have them, so they
        // are only printed for a package that is nothing but hooks, or on
        // request.
        if long || self.state == "hook" {
            match (self.has_setup, self.has_teardown) {
                (true, true) => parts.push("setup+teardown".to_string()),
                (true, false) => parts.push("setup".to_string()),
                (false, true) => parts.push("teardown".to_string()),
                (false, false) => {}
            }
        }

        if self.broken > 0 {
            parts.push(format!("{} broken", self.broken));
        }

        parts.join(", ")
    }
}

fn link_phrase(count: usize) -> String {
    format!("{} link{}", count, if count == 1 { "" } else { "s" })
}

/// Prints the one-line tally, so the common case is readable without scanning
/// every row.
fn print_summary(rows: &[(&str, std::result::Result<PackageStatus, String>)]) {
    let (mut ok, mut partial, mut missing, mut conflict, mut hook) = (0, 0, 0, 0, 0);
    let (mut broken, mut errors) = (0, 0);

    for (_, result) in rows {
        match result {
            Ok(status) => {
                match status.state {
                    "ok" => ok += 1,
                    "partial" => partial += 1,
                    "missing" => missing += 1,
                    "conflict" => conflict += 1,
                    "hook" => hook += 1,
                    _ => {}
                }
                broken += status.broken;
            }
            Err(_) => errors += 1,
        }
    }

    let mut summary = format!("{ok} ok, {partial} partial, {missing} missing, {conflict} conflict");
    if hook > 0 {
        summary.push_str(&format!(", {hook} hook-only"));
    }
    if broken > 0 {
        summary.push_str(&format!(
            "; {broken} broken link{}",
            if broken == 1 { "" } else { "s" }
        ));
    }
    if errors > 0 {
        summary.push_str(&format!(
            "; {errors} error{}",
            if errors == 1 { "" } else { "s" }
        ));
    }

    println!();
    println!("{summary}");
}
