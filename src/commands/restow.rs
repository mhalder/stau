use super::batch::{BatchLabels, run_for_packages};
use super::install::install_package;
use super::uninstall::{UninstallOptions, uninstall_package_internal};
use crate::config::Config;
use crate::error::Result;
use crate::options::RunOptions;

/// Restows a package by removing its symlinks without copying files back, then
/// installing it again.
pub fn restow_package(
    config: &Config,
    package: &str,
    no_setup: bool,
    no_teardown: bool,
    run: &RunOptions,
) -> Result<()> {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: false,
        dry_run: run.dry_run,
        verbose: run.verbose,
        json: run.json,
    };
    uninstall_package_internal(config, package, run.target.clone(), opts, &run.script)?;
    install_package(config, package, no_setup, run)
}

/// Restows every package in `packages`, in the order given, continuing past
/// failures and reporting a machine-readable summary when `run.json` is set.
pub fn restow_packages(
    config: &Config,
    packages: &[String],
    no_setup: bool,
    no_teardown: bool,
    run: &RunOptions,
) -> Result<()> {
    run_for_packages(
        config,
        packages,
        run,
        &BatchLabels::new("Restowing", "restow", "restowed", "Restowed"),
        |pkg| restow_package(config, pkg, no_setup, no_teardown, run),
    )
}
