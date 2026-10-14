use super::batch::{BatchLabels, PackageOutcome, run_for_packages};
use crate::config::Config;
use crate::error::{self, Result};
use crate::options::RunOptions;
use crate::symlink::MappingKind;
use crate::{package, script, symlink};
use std::path::{Path, PathBuf};

/// Options for uninstall operations.
pub struct UninstallOptions {
    /// Skip running the teardown script
    pub no_teardown: bool,
    /// Copy files back from package to target after removing symlinks
    pub copy_files_back: bool,
    /// Dry run mode - don't make actual changes
    pub dry_run: bool,
    /// Verbose output
    pub verbose: bool,
    /// Suppress stau's own human stdout output (JSON summary mode)
    pub json: bool,
}

/// Uninstalls a package by removing symlinks and optionally copying files back.
///
/// If a teardown script exists and `no_teardown` is false, it will be executed
/// first. A teardown failure is non-fatal: the uninstall carries on and the
/// outcome's `teardown_failed` flag records it for the machine-readable summary.
pub fn uninstall_package(
    config: &Config,
    package: &str,
    no_teardown: bool,
    run: &RunOptions,
) -> PackageOutcome {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: true,
        dry_run: run.dry_run,
        verbose: run.verbose,
        json: run.json,
    };
    uninstall_package_internal(
        config,
        package,
        run.target.clone(),
        opts,
        &run.script,
        &run.data,
    )
}

/// Uninstalls every package in `packages`, in the order given, continuing past
/// failures and reporting a machine-readable summary when `run.json` is set.
pub fn uninstall_packages(
    config: &Config,
    packages: &[String],
    no_teardown: bool,
    run: &RunOptions,
) -> error::Result<()> {
    run_for_packages(
        config,
        packages,
        run,
        &BatchLabels::new("Uninstalling", "uninstall", "uninstalled", "Uninstalled"),
        |pkg| uninstall_package(config, pkg, no_teardown, run),
    )
}

/// Internal implementation of package uninstallation with configurable options.
pub fn uninstall_package_internal(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    opts: UninstallOptions,
    script_options: &crate::script::ScriptOptions,
    data: &serde_json::Value,
) -> PackageOutcome {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if opts.verbose && !opts.json {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    if !config.package_exists(package) {
        return PackageOutcome::failed(error::StauError::PackageNotFound(package.to_string()));
    }

    let teardown_failed = run_teardown_script(config, package, &target_dir, &opts, script_options);

    let mappings = match package::discover_package_files(&package_dir, &target_dir, data) {
        Ok(mappings) => mappings,
        Err(e) => {
            return PackageOutcome {
                error: Some(e),
                teardown_failed,
            };
        }
    };
    if opts.verbose && !opts.json {
        let has_render = mappings.iter().any(|m| m.kind == MappingKind::Render);
        if has_render {
            println!("Found {} files to remove", mappings.len());
        } else {
            println!("Found {} symlinks to remove", mappings.len());
        }
    }

    if mappings.is_empty() {
        if !opts.json {
            println!("No symlinks to remove for package '{}'", package);
        }
        return PackageOutcome {
            error: None,
            teardown_failed,
        };
    }

    match remove_package_symlinks(&mappings, &opts) {
        Ok((symlinks, renders)) => {
            print_uninstall_summary(package, symlinks, renders, &opts);
            PackageOutcome {
                error: None,
                teardown_failed,
            }
        }
        Err(e) => PackageOutcome {
            error: Some(e),
            teardown_failed,
        },
    }
}

/// Runs the teardown script, returning `true` when it failed (non-fatal).
fn run_teardown_script(
    config: &Config,
    package: &str,
    target_dir: &Path,
    opts: &UninstallOptions,
    script_options: &crate::script::ScriptOptions,
) -> bool {
    if opts.no_teardown {
        return false;
    }

    let Some(teardown_script) = config.get_teardown_script(package) else {
        return false;
    };

    if opts.verbose && !opts.json {
        println!("Found teardown script: {}", teardown_script.display());
    }

    match script::execute_script(
        &teardown_script,
        package,
        &config.stau_dir,
        target_dir,
        opts.dry_run,
        opts.verbose,
        script_options,
    ) {
        Err(e) => {
            eprintln!("Warning: Teardown script failed: {}", e);
            eprintln!("Continuing with uninstall...");
            true
        }
        Ok(()) if !opts.dry_run && !opts.json => {
            println!("Teardown script completed successfully");
            false
        }
        Ok(()) => false,
    }
}

/// Removes every stau symlink and rendered file of a package, continuing past
/// failures so one bad file does not leave the rest of the package installed.
///
/// Every failure is reported; the last one is returned so its exit code is used.
/// Returns `(symlinks_removed, renders_removed)`.
fn remove_package_symlinks(
    mappings: &[symlink::SymlinkMapping],
    opts: &UninstallOptions,
) -> Result<(usize, usize)> {
    let mut symlinks = 0;
    let mut renders = 0;
    let mut errors = Vec::new();

    for mapping in mappings {
        match remove_package_symlink(mapping, opts) {
            Ok(true) => {
                if mapping.kind == MappingKind::Symlink {
                    symlinks += 1;
                } else {
                    renders += 1;
                }
            }
            Ok(false) => {}
            Err(e) => errors.push(e),
        }
    }

    let Some(last_error) = errors.pop() else {
        return Ok((symlinks, renders));
    };

    for e in &errors {
        eprintln!("Error: {}", e);
    }
    Err(last_error)
}

fn remove_package_symlink(
    mapping: &symlink::SymlinkMapping,
    opts: &UninstallOptions,
) -> Result<bool> {
    match mapping.kind {
        MappingKind::Symlink => remove_symlink(mapping, opts),
        MappingKind::Render => {
            let content = mapping
                .rendered
                .as_deref()
                .expect("render mapping carries content");
            if !opts.dry_run && symlink::rendered_differs(&mapping.target, content)? {
                eprintln!(
                    "Warning: discarding local modifications to {} (uninstall removes rendered files)",
                    mapping.target.display()
                );
            }
            if (opts.verbose || opts.dry_run) && !opts.json {
                println!("  Removing rendered file: {}", mapping.target.display());
            }
            symlink::remove_rendered(&mapping.target, opts.dry_run)
        }
    }
}

fn remove_symlink(mapping: &symlink::SymlinkMapping, opts: &UninstallOptions) -> Result<bool> {
    let was_removed = symlink::remove_symlink(&mapping.target, &mapping.source, opts.dry_run)?;

    if !was_removed {
        if opts.verbose && !opts.json {
            println!(
                "  Skipping {} (not a stau-managed symlink)",
                mapping.target.display()
            );
        }
        return Ok(false);
    }

    if (opts.verbose || opts.dry_run) && !opts.json {
        println!("  Removing symlink: {}", mapping.target.display());
    }

    if opts.copy_files_back
        && let Err(e) = copy_source_to_target(mapping, opts)
    {
        // The symlink is already gone; put it back so a failed copy does not
        // leave the target file missing
        if !opts.dry_run
            && let Err(restore_error) =
                symlink::create_symlink(&mapping.source, &mapping.target, false)
        {
            eprintln!(
                "Warning: Could not restore symlink {}: {}",
                mapping.target.display(),
                restore_error
            );
        }
        return Err(e);
    }

    Ok(true)
}

fn copy_source_to_target(mapping: &symlink::SymlinkMapping, opts: &UninstallOptions) -> Result<()> {
    if (opts.verbose || opts.dry_run) && !opts.json {
        println!("  Copying file: {}", mapping.target.display());
    }

    if !opts.dry_run && mapping.target.exists() {
        return Err(error::StauError::ConflictingFile(mapping.target.clone()));
    }

    if opts.dry_run && opts.verbose && !opts.json {
        println!("    (conflict check skipped in dry-run)");
    }

    symlink::copy_file(&mapping.source, &mapping.target, opts.dry_run)
}

fn print_uninstall_summary(
    package: &str,
    symlinks: usize,
    renders: usize,
    opts: &UninstallOptions,
) {
    if opts.dry_run || opts.json {
        return;
    }

    if opts.copy_files_back {
        if renders == 0 {
            println!(
                "Successfully uninstalled {} ({} symlinks removed, files copied back)",
                package, symlinks
            );
        } else {
            println!(
                "Successfully uninstalled {} ({} symlinks removed, files copied back, {} rendered files removed)",
                package, symlinks, renders
            );
        }
    } else if renders == 0 {
        println!("Successfully removed {} symlinks for {}", symlinks, package);
    } else {
        println!(
            "Successfully removed {} symlinks and {} rendered files for {}",
            symlinks, renders, package
        );
    }
}
