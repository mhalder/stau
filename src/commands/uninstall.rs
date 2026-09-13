use super::batch::{BatchLabels, run_for_all_packages};
use crate::config::Config;
use crate::error::{self, Result};
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
}

/// Uninstalls a package by removing symlinks and optionally copying files back.
///
/// If a teardown script exists and `no_teardown` is false, it will be executed first.
/// Teardown script failures are logged but don't stop the uninstall process.
pub fn uninstall_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: true,
        dry_run,
        verbose,
    };
    uninstall_package_internal(config, package, target, opts)
}

/// Uninstalls all packages found in STAU_DIR.
///
/// For each package, teardown scripts are run (unless skipped) and symlinks are removed.
/// If any package fails to uninstall, the error is reported but uninstallation continues
/// for remaining packages, and an error is returned once all packages were processed.
pub fn uninstall_all_packages(
    config: &Config,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    run_for_all_packages(
        config,
        target,
        verbose,
        BatchLabels::new("Uninstalling", "uninstall", "uninstalled", "Uninstalled"),
        |pkg, target| uninstall_package(config, pkg, target, no_teardown, dry_run, verbose),
    )
}

/// Internal implementation of package uninstallation with configurable options.
pub fn uninstall_package_internal(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    opts: UninstallOptions,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if opts.verbose {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    run_teardown_script(config, package, &target_dir, &opts);

    let mappings = package::discover_package_files(&package_dir, &target_dir)?;
    if opts.verbose {
        println!("Found {} symlinks to remove", mappings.len());
    }

    if mappings.is_empty() {
        println!("No symlinks to remove for package '{}'", package);
        return Ok(());
    }

    let removed_count = remove_package_symlinks(&mappings, &opts)?;
    print_uninstall_summary(package, removed_count, &opts);
    Ok(())
}

fn run_teardown_script(config: &Config, package: &str, target_dir: &Path, opts: &UninstallOptions) {
    if opts.no_teardown {
        return;
    }

    let Some(teardown_script) = config.get_teardown_script(package) else {
        return;
    };

    if opts.verbose {
        println!("Found teardown script: {}", teardown_script.display());
    }

    match script::execute_script(
        &teardown_script,
        package,
        &config.stau_dir,
        target_dir,
        opts.dry_run,
        opts.verbose,
    ) {
        Err(e) => {
            eprintln!("Warning: Teardown script failed: {}", e);
            eprintln!("Continuing with uninstall...");
        }
        Ok(()) if !opts.dry_run => println!("Teardown script completed successfully"),
        Ok(()) => {}
    }
}

/// Removes every stau symlink of a package, continuing past failures so one bad file
/// does not leave the rest of the package installed.
///
/// Every failure is reported; the last one is returned so its exit code is used.
fn remove_package_symlinks(
    mappings: &[symlink::SymlinkMapping],
    opts: &UninstallOptions,
) -> Result<usize> {
    let mut removed_count = 0;
    let mut errors = Vec::new();

    for mapping in mappings {
        match remove_package_symlink(mapping, opts) {
            Ok(true) => removed_count += 1,
            Ok(false) => {}
            Err(e) => errors.push(e),
        }
    }

    let Some(last_error) = errors.pop() else {
        return Ok(removed_count);
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
    let was_removed = symlink::remove_symlink(&mapping.target, &mapping.source, opts.dry_run)?;

    if !was_removed {
        if opts.verbose {
            println!(
                "  Skipping {} (not a stau-managed symlink)",
                mapping.target.display()
            );
        }
        return Ok(false);
    }

    if opts.verbose || opts.dry_run {
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
    if opts.verbose || opts.dry_run {
        println!("  Copying file: {}", mapping.target.display());
    }

    if !opts.dry_run && mapping.target.exists() {
        return Err(error::StauError::ConflictingFile(mapping.target.clone()));
    }

    if opts.dry_run && opts.verbose {
        println!("    (conflict check skipped in dry-run)");
    }

    symlink::copy_file(&mapping.source, &mapping.target, opts.dry_run)
}

fn print_uninstall_summary(package: &str, removed_count: usize, opts: &UninstallOptions) {
    if opts.dry_run {
        return;
    }

    if opts.copy_files_back {
        println!(
            "Successfully uninstalled {} ({} symlinks removed, files copied back)",
            package, removed_count
        );
    } else {
        println!(
            "Successfully removed {} symlinks for {}",
            removed_count, package
        );
    }
}
