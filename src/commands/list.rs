use crate::config::Config;
use crate::error::Result;
use crate::{package, symlink};
use std::path::{Path, PathBuf};

/// Lists all packages in STAU_DIR and their installation status.
pub fn list_packages(config: &Config, target: Option<PathBuf>) -> Result<()> {
    let target_dir = config.get_target(target);
    let packages = package::list_packages(&config.stau_dir)?;

    if packages.is_empty() {
        println!("No packages found in {}", config.stau_dir.display());
        return Ok(());
    }

    println!("Packages in {}:\n", config.stau_dir.display());
    for pkg in packages {
        print_package_status(config, &target_dir, &pkg);
    }

    Ok(())
}

struct PackageStatus {
    total_count: usize,
    installed_count: usize,
    broken_count: usize,
}

fn print_package_status(config: &Config, target_dir: &Path, pkg: &str) {
    match package_status(&config.get_package_dir(pkg), target_dir) {
        Ok(status) => print_package_status_line(pkg, &status),
        Err(e) => println!("  {:<20} [error: {}]", pkg, e),
    }
}

fn print_package_status_line(pkg: &str, status: &PackageStatus) {
    let broken = if status.broken_count > 0 {
        format!("  ({} broken)", status.broken_count)
    } else {
        String::new()
    };

    if status.installed_count == 0 {
        println!("  {:<20} [not installed]{}", pkg, broken);
    } else if status.installed_count == status.total_count {
        println!(
            "  {:<20} [installed]  {} symlink{}{}",
            pkg,
            status.installed_count,
            plural_suffix(status.installed_count),
            broken
        );
    } else {
        println!(
            "  {:<20} [partial]    {}/{} symlinks{}",
            pkg, status.installed_count, status.total_count, broken
        );
    }
}

fn package_status(package_dir: &Path, target_dir: &Path) -> Result<PackageStatus> {
    let mappings = package::discover_package_files(package_dir, target_dir)?;
    let installed_count = mappings
        .iter()
        .filter(|mapping| {
            matches!(
                symlink::is_stau_symlink(&mapping.target, &mapping.source),
                Ok(true)
            )
        })
        .count();

    Ok(PackageStatus {
        total_count: mappings.len(),
        installed_count,
        broken_count: package::discover_broken_symlinks(package_dir, target_dir)?.len(),
    })
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
