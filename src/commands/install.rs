use super::batch::{BatchLabels, run_for_all_packages};
use crate::config::Config;
use crate::error::{self, Result};
use crate::{package, script, symlink};
use std::path::PathBuf;

/// Installs a package by creating symlinks from the package directory to the target directory.
///
/// If a setup script exists and `no_setup` is false, it will be executed after creating symlinks.
pub fn install_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if verbose {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    // Check if package exists
    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    // Discover all files in the package
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;

    if verbose {
        println!("Found {} files to link", mappings.len());
    }

    if mappings.is_empty() {
        println!("No files to link in package '{}'", package);
    } else {
        // Check every target for conflicts before creating any symlink, so a
        // conflict does not leave the package partially installed
        for mapping in &mappings {
            symlink::create_symlink(&mapping.source, &mapping.target, true)?;
        }

        // Create symlinks for all files
        for mapping in &mappings {
            if verbose || dry_run {
                println!(
                    "  {} -> {}",
                    mapping.target.display(),
                    mapping.source.display()
                );
            }

            symlink::create_symlink(&mapping.source, &mapping.target, dry_run)?;
        }

        if !dry_run {
            println!(
                "Successfully installed {} ({} symlinks created)",
                package,
                mappings.len()
            );
        }
    }

    // Run setup script if it exists and not skipped
    if !no_setup && let Some(setup_script) = config.get_setup_script(package) {
        if verbose {
            println!("Found setup script: {}", setup_script.display());
        }

        script::execute_script(
            &setup_script,
            package,
            &config.stau_dir,
            &target_dir,
            dry_run,
            verbose,
        )?;

        if !dry_run {
            println!("Setup script completed successfully");
        }
    }

    Ok(())
}

/// Installs all packages found in STAU_DIR.
///
/// For each package, symlinks are created and setup scripts are run (unless skipped).
/// If any package fails to install, the error is reported but installation continues
/// for remaining packages, and an error is returned once all packages were processed.
pub fn install_all_packages(
    config: &Config,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    run_for_all_packages(
        config,
        target,
        verbose,
        BatchLabels::new("Installing", "install", "installed", "Installed"),
        |pkg, target| install_package(config, pkg, target, no_setup, dry_run, verbose),
    )
}
