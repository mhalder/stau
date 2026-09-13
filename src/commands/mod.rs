mod adopt;
mod batch;
mod clean;
mod install;
mod list;
mod restow;
mod status;
mod uninstall;

use crate::cli::Commands;
use crate::config::Config;
use crate::error::{self, Result};
use std::path::PathBuf;

/// Validates the package name given on the command line, then runs the command.
pub fn dispatch(config: &Config, command: Commands, dry_run: bool, verbose: bool) -> Result<()> {
    if let Some(package) = command.package() {
        Config::validate_package_name(package)?;
    }

    match command {
        Commands::Install {
            package,
            all,
            target,
            no_setup,
        } => run_install(config, package, all, target, no_setup, dry_run, verbose),
        Commands::Uninstall {
            package,
            all,
            target,
            no_teardown,
        } => run_uninstall(config, package, all, target, no_teardown, dry_run, verbose),
        Commands::Restow {
            package,
            target,
            no_setup,
            no_teardown,
        } => restow::restow_package(
            config,
            &package,
            target,
            no_setup,
            no_teardown,
            dry_run,
            verbose,
        ),
        Commands::Adopt {
            package,
            files,
            target,
        } => adopt::adopt_files(config, &package, &files, target, dry_run, verbose),
        Commands::List { target } => list::list_packages(config, target),
        Commands::Status { package, target } => status::show_status(config, &package, target),
        Commands::Clean {
            package,
            target,
            deep,
        } => clean::clean_broken_symlinks(config, &package, target, deep, dry_run, verbose),
    }
}

fn require_single_or_all(package: Option<String>, all: bool) -> Result<Option<String>> {
    match (all, package) {
        (true, Some(_)) => Err(error::StauError::InvalidArguments(
            "Cannot specify both --all and a package name".to_string(),
        )),
        (false, None) => Err(error::StauError::InvalidArguments(
            "Must specify either a package name or --all".to_string(),
        )),
        (_, package) => Ok(package),
    }
}

fn run_install(
    config: &Config,
    package: Option<String>,
    all: bool,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let package = require_single_or_all(package, all)?;

    if all {
        install::install_all_packages(config, target, no_setup, dry_run, verbose)
    } else {
        install::install_package(
            config,
            &package.expect("validated package"),
            target,
            no_setup,
            dry_run,
            verbose,
        )
    }
}

fn run_uninstall(
    config: &Config,
    package: Option<String>,
    all: bool,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let package = require_single_or_all(package, all)?;

    if all {
        uninstall::uninstall_all_packages(config, target, no_teardown, dry_run, verbose)
    } else {
        uninstall::uninstall_package(
            config,
            &package.expect("validated package"),
            target,
            no_teardown,
            dry_run,
            verbose,
        )
    }
}
