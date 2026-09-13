use crate::config::Config;
use crate::error::{self, Result, map_io_error};
use crate::package;
use std::fs;
use std::path::PathBuf;

/// Removes broken symlinks that point into a package.
///
/// With `deep`, the whole target tree is scanned, which also finds links below
/// directories removed from the package entirely.
pub fn clean_broken_symlinks(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    deep: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    let broken = if deep {
        package::discover_broken_symlinks_deep(&package_dir, &target_dir)?
    } else {
        package::discover_broken_symlinks(&package_dir, &target_dir)?
    };

    for path in &broken {
        if verbose || dry_run {
            println!("  Removing broken symlink: {}", path.display());
        }

        if !dry_run {
            fs::remove_file(path).map_err(|e| {
                map_io_error(e, format!("Cannot remove symlink: {}", path.display()))
            })?;
        }
    }

    if broken.is_empty() {
        println!("No broken symlinks found for package '{}'", package);
    } else if !dry_run {
        println!(
            "Cleaned {} broken symlink(s) for package '{}'",
            broken.len(),
            package
        );
    }

    Ok(())
}
