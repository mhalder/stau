use crate::config::Config;
use crate::error::{self, Result};
use crate::{package, symlink};
use std::path::PathBuf;

/// Shows detailed status for a specific package including symlink states and script availability.
pub fn show_status(config: &Config, package: &str, target: Option<PathBuf>) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    println!("Status for package '{}':\n", package);
    println!("  Package directory: {}", package_dir.display());
    println!("  Target directory:  {}", target_dir.display());

    // Check for setup/teardown scripts
    if let Some(setup) = config.get_setup_script(package) {
        println!("  Setup script:      {} (exists)", setup.display());
    } else {
        println!("  Setup script:      (none)");
    }

    if let Some(teardown) = config.get_teardown_script(package) {
        println!("  Teardown script:   {} (exists)", teardown.display());
    } else {
        println!("  Teardown script:   (none)");
    }

    // Get all mappings
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;
    let broken = package::discover_broken_symlinks(&package_dir, &target_dir)?;

    if mappings.is_empty() && broken.is_empty() {
        println!("\nNo files in package.");
        return Ok(());
    }

    println!("\nFiles ({} total):", mappings.len());

    let mut installed = 0;
    let mut not_installed = 0;

    for mapping in &mappings {
        let status = if broken.contains(&mapping.target) {
            "[BROKEN]"
        } else if symlink::is_stau_symlink(&mapping.target, &mapping.source)? {
            installed += 1;
            "[installed]"
        } else if mapping.target.symlink_metadata().is_ok() {
            // Includes dangling symlinks that do not point into the package
            not_installed += 1;
            "[conflict]"
        } else {
            not_installed += 1;
            "[not installed]"
        };

        println!("  {:<20} {}", status, mapping.target.display());
    }

    // Broken symlinks to files that were removed from the package
    for path in broken
        .iter()
        .filter(|path| !mappings.iter().any(|mapping| mapping.target == **path))
    {
        println!("  {:<20} {}", "[BROKEN]", path.display());
    }

    println!();
    println!(
        "Summary: {} installed, {} not installed, {} broken",
        installed,
        not_installed,
        broken.len()
    );

    Ok(())
}
