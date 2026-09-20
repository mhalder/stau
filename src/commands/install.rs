use super::batch::{BatchLabels, run_for_packages};
use crate::config::Config;
use crate::error::{self, Result};
use crate::options::RunOptions;
use crate::{package, script, symlink};

/// Installs a package by creating symlinks from the package directory to the target directory.
///
/// If a setup script exists and `no_setup` is false, it will be executed after creating symlinks.
///
/// With `run.json`, stau's own human output is suppressed so a machine-readable
/// summary can be printed by the caller.
pub fn install_package(
    config: &Config,
    package: &str,
    no_setup: bool,
    run: &RunOptions,
) -> Result<()> {
    let target_dir = config.get_target(run.target.clone());
    let package_dir = config.get_package_dir(package);

    if run.verbose && !run.json {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    // Check if package exists
    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    // Discover all files in the package
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;

    if run.verbose && !run.json {
        println!("Found {} files to link", mappings.len());
    }

    if mappings.is_empty() {
        if !run.json {
            println!("No files to link in package '{}'", package);
        }
    } else {
        // Check every target for conflicts before creating any symlink, so a
        // conflict does not leave the package partially installed
        symlink::check_conflicts(&mappings)?;

        // Create symlinks for all files
        for mapping in &mappings {
            if (run.verbose || run.dry_run) && !run.json {
                println!(
                    "  {} -> {}",
                    mapping.target.display(),
                    mapping.source.display()
                );
            }

            symlink::create_symlink(&mapping.source, &mapping.target, run.dry_run)?;
        }

        if !run.dry_run && !run.json {
            println!(
                "Successfully installed {} ({} symlinks created)",
                package,
                mappings.len()
            );
        }
    }

    // Run setup script if it exists and not skipped
    if !no_setup && let Some(setup_script) = config.get_setup_script(package) {
        if run.verbose && !run.json {
            println!("Found setup script: {}", setup_script.display());
        }

        script::execute_script(
            &setup_script,
            package,
            &config.stau_dir,
            &target_dir,
            run.dry_run,
            run.verbose,
            &run.script,
        )?;

        if !run.dry_run && !run.json {
            println!("Setup script completed successfully");
        }
    }

    Ok(())
}

/// Installs every package in `packages`, in the order given, continuing past
/// failures and reporting a machine-readable summary when `run.json` is set.
pub fn install_packages(
    config: &Config,
    packages: &[String],
    no_setup: bool,
    run: &RunOptions,
) -> Result<()> {
    run_for_packages(
        config,
        packages,
        run,
        &BatchLabels::new("Installing", "install", "installed", "Installed"),
        |pkg| install_package(config, pkg, no_setup, run),
    )
}
