use super::batch::{BatchLabels, PackageOutcome, run_for_packages};
use super::install::install_package;
use super::uninstall::{UninstallOptions, uninstall_package_internal};
use crate::config::Config;
use crate::error;
use crate::options::RunOptions;
use crate::{package, template};

/// Restows a package by removing its symlinks without copying files back, then
/// installing it again.
pub fn restow_package(
    config: &Config,
    package: &str,
    no_setup: bool,
    no_teardown: bool,
    run: &RunOptions,
) -> PackageOutcome {
    // Render every template up front: a broken template must fail the restow
    // before the uninstall phase, so it cannot leave the package half-removed.
    let target_dir = config.get_target(run.target.clone());
    let package_dir = config.get_package_dir(package);
    if !config.package_exists(package) {
        return PackageOutcome::failed(error::StauError::PackageNotFound(package.to_string()));
    }
    let mut mappings = match package::discover_package_files(&package_dir, &target_dir) {
        Ok(mappings) => mappings,
        Err(e) => return PackageOutcome::failed(e),
    };
    template::render_mappings(&mut mappings, &run.data);
    if let Some(e) = template::collect_render_errors(&mappings) {
        return PackageOutcome::failed(e);
    }

    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: false,
        dry_run: run.dry_run,
        verbose: run.verbose,
        json: run.json,
    };
    let uninstall = uninstall_package_internal(
        config,
        package,
        run.target.clone(),
        opts,
        &run.script,
        &run.data,
    );

    // A hard uninstall failure aborts the restow, as before; a teardown failure
    // only carries its flag forward into the install.
    if let Some(e) = uninstall.error {
        return PackageOutcome {
            error: Some(e),
            teardown_failed: uninstall.teardown_failed,
        };
    }

    let install = install_package(config, package, no_setup, run);
    PackageOutcome {
        error: install.error,
        teardown_failed: uninstall.teardown_failed,
    }
}

/// Restows every package in `packages`, in the order given, continuing past
/// failures and reporting a machine-readable summary when `run.json` is set.
pub fn restow_packages(
    config: &Config,
    packages: &[String],
    no_setup: bool,
    no_teardown: bool,
    run: &RunOptions,
) -> error::Result<()> {
    run_for_packages(
        config,
        packages,
        run,
        &BatchLabels::new("Restowing", "restow", "restowed", "Restowed"),
        |pkg| restow_package(config, pkg, no_setup, no_teardown, run),
    )
}
