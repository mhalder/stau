use super::batch::{BatchLabels, PackageOutcome, run_for_packages};
use crate::config::Config;
use crate::error::{self, StauError};
use crate::options::RunOptions;
use crate::symlink::MappingKind;
use crate::{package, script, symlink, template};

/// Installs a package by creating symlinks from the package directory to the target directory.
///
/// If a setup script exists and `no_setup` is false, it will be executed after creating symlinks.
///
/// With `refresh_rendered`, a rendered target whose bytes drifted from the
/// current render is re-rendered in place instead of conflicting.
///
/// With `run.json`, stau's own human output is suppressed and the hook's stdout
/// is redirected to stderr so a machine-readable summary can be printed by the caller.
pub fn install_package(
    config: &Config,
    package: &str,
    no_setup: bool,
    refresh_rendered: bool,
    run: &RunOptions,
) -> PackageOutcome {
    let target_dir = config.get_target(run.target.clone());
    let package_dir = config.get_package_dir(package);

    if run.verbose && !run.json {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    // Check if package exists
    if !config.package_exists(package) {
        return PackageOutcome::failed(StauError::PackageNotFound(package.to_string()));
    }

    // Discover all files in the package
    let mut mappings = match package::discover_package_files(&package_dir, &target_dir) {
        Ok(mappings) => mappings,
        Err(e) => return PackageOutcome::failed(e),
    };

    // Render every template up front; a broken template must fail the whole
    // package before any symlink or file is created (no partial install).
    template::render_mappings(&mut mappings, &run.data);
    if let Some(e) = template::collect_render_errors(&mappings) {
        return PackageOutcome::failed(e);
    }

    if run.verbose && !run.json {
        println!("Found {} files to link", mappings.len());
    }

    let mut refreshed = 0usize;

    if mappings.is_empty() {
        if !run.json {
            println!("No files to link in package '{}'", package);
        }
    } else {
        // Check every target for conflicts before creating any link or file, so a
        // conflict does not leave the package partially installed
        if let Err(e) = symlink::check_conflicts(&mappings, refresh_rendered) {
            return PackageOutcome::failed(e);
        }

        // Create symlinks / write rendered files for all mappings
        for mapping in &mappings {
            match mapping.kind {
                MappingKind::Symlink => {
                    if (run.verbose || run.dry_run) && !run.json {
                        println!(
                            "  {} -> {}",
                            mapping.target.display(),
                            mapping.source.display()
                        );
                    }
                    if let Err(e) =
                        symlink::create_symlink(&mapping.source, &mapping.target, run.dry_run)
                    {
                        return PackageOutcome::failed(e);
                    }
                }
                MappingKind::Render => {
                    let content = mapping
                        .rendered
                        .as_deref()
                        .expect("render mapping carries content");
                    let refresh = refresh_rendered
                        && symlink::rendered_differs(&mapping.target, content).unwrap_or(false);
                    if (run.verbose || run.dry_run) && !run.json {
                        if refresh {
                            println!("  {} (would refresh)", mapping.target.display());
                        } else if symlink::rendered_is_current(&mapping.target, content)
                            .unwrap_or(false)
                        {
                            println!("  {} (up to date)", mapping.target.display());
                        } else {
                            println!(
                                "  {} <- {}",
                                mapping.target.display(),
                                mapping.source.display()
                            );
                        }
                    }
                    let result = if refresh {
                        symlink::refresh_rendered(
                            &mapping.source,
                            &mapping.target,
                            content,
                            run.dry_run,
                        )
                    } else {
                        symlink::write_rendered(
                            &mapping.source,
                            &mapping.target,
                            content,
                            run.dry_run,
                        )
                    };
                    if let Err(e) = result {
                        return PackageOutcome::failed(e);
                    }
                    if refresh {
                        refreshed += 1;
                    }
                }
            }
        }

        if !run.dry_run && !run.json {
            let symlinks = mappings
                .iter()
                .filter(|m| m.kind == MappingKind::Symlink)
                .count();
            let renders = mappings
                .iter()
                .filter(|m| m.kind == MappingKind::Render)
                .count();
            let mut message = match (symlinks, renders) {
                (_, 0) => format!(
                    "Successfully installed {} ({} symlinks created)",
                    package, symlinks
                ),
                (0, _) => format!(
                    "Successfully installed {} ({} files rendered)",
                    package, renders
                ),
                (s, r) => format!(
                    "Successfully installed {} ({} symlinks created, {} files rendered)",
                    package, s, r
                ),
            };
            if refreshed > 0 {
                message.push_str(&format!(", {} refreshed", refreshed));
            }
            println!("{message}");
        }
    }

    // Run setup script if it exists and not skipped
    if !no_setup && let Some(setup_script) = config.get_setup_script(package) {
        if run.verbose && !run.json {
            println!("Found setup script: {}", setup_script.display());
        }

        if let Err(e) = script::execute_script(
            &setup_script,
            package,
            &config.stau_dir,
            &target_dir,
            run.dry_run,
            run.verbose,
            &run.script,
        ) {
            return PackageOutcome::failed(e);
        }

        if !run.dry_run && !run.json {
            println!("Setup script completed successfully");
        }
    }

    let mut outcome = PackageOutcome::ok();
    outcome.rendered_refreshed = Some(refreshed);
    outcome
}

/// Installs every package in `packages`, in the order given, continuing past
/// failures and reporting a machine-readable summary when `run.json` is set.
pub fn install_packages(
    config: &Config,
    packages: &[String],
    no_setup: bool,
    refresh_rendered: bool,
    run: &RunOptions,
) -> error::Result<()> {
    run_for_packages(
        config,
        packages,
        run,
        &BatchLabels::new("Installing", "install", "installed", "Installed"),
        |pkg| install_package(config, pkg, no_setup, refresh_rendered, run),
    )
}
