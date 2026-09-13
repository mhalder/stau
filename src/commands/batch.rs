use crate::config::Config;
use crate::error::{self, Result};
use crate::package;
use std::path::PathBuf;

pub struct BatchLabels {
    progress: &'static str,
    action: &'static str,
    success: &'static str,
    partial: &'static str,
}

impl BatchLabels {
    pub fn new(
        progress: &'static str,
        action: &'static str,
        success: &'static str,
        partial: &'static str,
    ) -> Self {
        Self {
            progress,
            action,
            success,
            partial,
        }
    }
}

pub fn run_for_all_packages<F>(
    config: &Config,
    target: Option<PathBuf>,
    verbose: bool,
    labels: BatchLabels,
    mut apply: F,
) -> Result<()>
where
    F: FnMut(&str, Option<PathBuf>) -> Result<()>,
{
    let packages = package::list_packages(&config.stau_dir)?;

    if packages.is_empty() {
        println!("No packages found in {}", config.stau_dir.display());
        return Ok(());
    }

    println!("{} {} packages...\n", labels.progress, packages.len());
    let (success_count, error_count) =
        apply_to_packages(&packages, target, verbose, &labels, &mut apply);
    print_batch_summary(success_count, error_count, &labels);

    if error_count > 0 {
        return Err(error::StauError::Other(format!(
            "Failed to {} {} of {} packages",
            labels.action,
            error_count,
            packages.len()
        )));
    }

    Ok(())
}

fn apply_to_packages<F>(
    packages: &[String],
    target: Option<PathBuf>,
    verbose: bool,
    labels: &BatchLabels,
    apply: &mut F,
) -> (usize, usize)
where
    F: FnMut(&str, Option<PathBuf>) -> Result<()>,
{
    let mut success_count = 0;
    let mut error_count = 0;

    for pkg in packages {
        if verbose {
            println!("--- {} package: {} ---", labels.progress, pkg);
        }

        match apply(pkg, target.clone()) {
            Ok(()) => success_count += 1,
            Err(e) => {
                eprintln!("Failed to {} '{}': {}", labels.action, pkg, e);
                error_count += 1;
            }
        }

        if verbose {
            println!();
        }
    }

    (success_count, error_count)
}

fn print_batch_summary(success_count: usize, error_count: usize, labels: &BatchLabels) {
    println!();
    if error_count == 0 {
        println!(
            "Successfully {} all {} packages",
            labels.success, success_count
        );
    } else {
        println!(
            "{} {} packages, {} failed",
            labels.partial, success_count, error_count
        );
    }
}
