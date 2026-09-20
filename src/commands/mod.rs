mod adopt;
mod batch;
mod clean;
mod install;
mod list;
mod restow;
mod status;
mod uninstall;
mod verify;

use crate::cli::{Commands, ScriptArgs};
use crate::config::Config;
use crate::error::{self, Result};
use crate::options::RunOptions;
use crate::script::ScriptOptions;

/// Validates the package names and runs the command.
pub fn dispatch(config: &Config, command: Commands, dry_run: bool, verbose: bool) -> Result<()> {
    match command {
        Commands::Install {
            package,
            all,
            profile,
            target,
            no_setup,
            json,
            script,
        } => {
            let packages = resolve_packages(config, package, all, profile)?;
            let run = RunOptions {
                target,
                json,
                dry_run,
                verbose,
                script: script_options(&script)?,
            };
            install::install_packages(config, &packages, no_setup, &run)
        }
        Commands::Uninstall {
            package,
            all,
            profile,
            target,
            no_teardown,
            json,
            script,
        } => {
            let packages = resolve_packages(config, package, all, profile)?;
            let run = RunOptions {
                target,
                json,
                dry_run,
                verbose,
                script: script_options(&script)?,
            };
            uninstall::uninstall_packages(config, &packages, no_teardown, &run)
        }
        Commands::Restow {
            package,
            all,
            profile,
            target,
            no_setup,
            no_teardown,
            json,
            script,
        } => {
            let packages = resolve_packages(config, package, all, profile)?;
            let run = RunOptions {
                target,
                json,
                dry_run,
                verbose,
                script: script_options(&script)?,
            };
            restow::restow_packages(config, &packages, no_setup, no_teardown, &run)
        }
        Commands::Adopt {
            package,
            files,
            target,
        } => {
            Config::validate_package_name(&package)?;
            adopt::adopt_files(config, &package, &files, target, dry_run, verbose)
        }
        Commands::List { target } => list::list_packages(config, target),
        Commands::Status {
            package,
            target,
            json,
        } => status::run_status(config, package, target, json),
        Commands::Verify {
            package,
            expect,
            target,
            json,
        } => verify::run_verify(config, &package, expect, target, json),
        Commands::Clean {
            package,
            target,
            deep,
        } => {
            Config::validate_package_name(&package)?;
            clean::clean_broken_symlinks(config, &package, target, deep, dry_run, verbose)
        }
    }
}

/// Turns the parsed `--set`, `--non-interactive`, and `--script-timeout` flags
/// into the options passed to every hook script.
fn script_options(args: &ScriptArgs) -> Result<ScriptOptions> {
    let mut env = Vec::with_capacity(args.set.len());
    for pair in &args.set {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(error::StauError::InvalidArguments(format!(
                "--set expects KEY=VALUE, got {pair:?}"
            )));
        };
        if key.is_empty() {
            return Err(error::StauError::InvalidArguments(
                "--set expects a non-empty KEY".to_string(),
            ));
        }
        env.push((key.to_string(), value.to_string()));
    }

    let timeout = args
        .script_timeout
        .as_deref()
        .map(crate::script::parse_duration)
        .transpose()
        .map_err(error::StauError::InvalidArguments)?;

    Ok(ScriptOptions {
        env,
        non_interactive: args.non_interactive,
        timeout,
    })
}

/// Resolves the ordered package list from an explicit list, `--all`, or a profile.
///
/// `--all` is alphabetical (it has no dependency information); the explicit
/// list and a profile preserve their order.
fn resolve_packages(
    config: &Config,
    packages: Vec<String>,
    all: bool,
    profile: Option<String>,
) -> Result<Vec<String>> {
    let has_list = !packages.is_empty();
    let has_profile = profile.is_some();

    match (all, has_list, has_profile) {
        (true, true, _) => Err(error::StauError::InvalidArguments(
            "Cannot specify both --all and a package name".to_string(),
        )),
        (true, false, true) => Err(error::StauError::InvalidArguments(
            "Cannot specify both --all and --profile".to_string(),
        )),
        (false, true, true) => Err(error::StauError::InvalidArguments(
            "Cannot specify both a package list and --profile".to_string(),
        )),
        (false, false, false) => Err(error::StauError::InvalidArguments(
            "Must specify either a package name or --all".to_string(),
        )),
        (true, false, false) => crate::package::list_packages(&config.stau_dir),
        (false, false, true) => crate::profile::read_profile(config, &profile.expect("checked")),
        (false, true, false) => {
            for package in &packages {
                Config::validate_package_name(package)?;
            }
            Ok(packages)
        }
    }
}
