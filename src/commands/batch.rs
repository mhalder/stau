use crate::config::Config;
use crate::error::{self, Result};
use crate::options::RunOptions;
use serde::Serialize;

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

/// The per-package outcome of a batch run.
#[derive(Serialize)]
struct PackageResult {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// The JSON summary printed for `--json` batch runs.
#[derive(Serialize)]
struct BatchSummary<'a> {
    ok: bool,
    packages: &'a [PackageResult],
}

/// Applies `apply` to every package in `packages`, in the order given, and
/// continues past failures.
///
/// With `json`, human progress output is suppressed and a JSON summary is
/// printed to stdout instead (also when some packages failed).
pub fn run_for_packages<F>(
    config: &Config,
    packages: &[String],
    run: &RunOptions,
    labels: &BatchLabels,
    mut apply: F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    if packages.is_empty() {
        if run.json {
            print_json_summary(true, &[]);
        } else {
            println!("No packages found in {}", config.stau_dir.display());
        }
        return Ok(());
    }

    // A single package without --json keeps the single-package output and, more
    // importantly, surfaces the original error so its exit code (conflict, I/O,
    // script failure) is preserved.
    if !run.json && packages.len() == 1 {
        return apply(&packages[0]);
    }

    if !run.json {
        println!("{} {} packages...\n", labels.progress, packages.len());
    }

    let mut results = Vec::with_capacity(packages.len());
    for pkg in packages {
        if run.verbose && !run.json {
            println!("--- {} package: {} ---", labels.progress, pkg);
        }

        match apply(pkg) {
            Ok(()) => results.push(PackageResult {
                name: pkg.clone(),
                ok: true,
                error: None,
            }),
            Err(e) => {
                if !run.json {
                    eprintln!("Failed to {} '{}': {}", labels.action, pkg, e);
                }
                results.push(PackageResult {
                    name: pkg.clone(),
                    ok: false,
                    error: Some(e.to_string()),
                });
            }
        }

        if run.verbose && !run.json {
            println!();
        }
    }

    let error_count = results.iter().filter(|r| !r.ok).count();
    let success_count = results.len() - error_count;

    if run.json {
        print_json_summary(error_count == 0, &results);
    } else {
        print_batch_summary(success_count, error_count, labels);
    }

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

fn print_json_summary(ok: bool, results: &[PackageResult]) {
    // The summary shape is fixed here and mirrors the status envelope's
    // machine-readable style; a failure to serialize would be a programming
    // error, so it is acceptable to panic.
    println!(
        "{}",
        serde_json::to_string(&BatchSummary {
            ok,
            packages: results
        })
        .expect("batch summary is always serializable")
    );
}
