use super::install::install_package;
use super::uninstall::{UninstallOptions, uninstall_package_internal};
use crate::config::Config;
use crate::error::Result;
use std::path::PathBuf;

/// Restows a package by removing its symlinks without copying files back, then
/// installing it again.
pub fn restow_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_setup: bool,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: false,
        dry_run,
        verbose,
    };
    uninstall_package_internal(config, package, target.clone(), opts)?;
    install_package(config, package, target, no_setup, dry_run, verbose)
}
