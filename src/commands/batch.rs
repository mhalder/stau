use crate::config::Config;
use crate::error::{self, Result, StauError};
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

/// What applying one package produced.
#[derive(Default)]
pub struct PackageOutcome {
    /// Fatal error (install/uninstall failed), if any.
    pub error: Option<StauError>,
    /// The teardown script failed but the uninstall carried on (non-fatal:
    /// symlinks were still removed and files copied back).
    pub teardown_failed: bool,
    /// The number of drifted rendered targets refreshed by an `install` run.
    pub rendered_refreshed: Option<usize>,
}

impl PackageOutcome {
    /// A successful outcome.
    pub fn ok() -> Self {
        Self::default()
    }

    /// A fatal failure.
    pub fn failed(error: StauError) -> Self {
        Self {
            error: Some(error),
            teardown_failed: false,
            rendered_refreshed: None,
        }
    }

    /// A fatal failure of an `install`, carrying how many drifted rendered
    /// targets were refreshed before the failure (0 for the early failures).
    pub fn install_failed(error: StauError, rendered_refreshed: usize) -> Self {
        Self {
            error: Some(error),
            teardown_failed: false,
            rendered_refreshed: Some(rendered_refreshed),
        }
    }
}

/// The per-package outcome of a batch run, as serialized for `--json`.
#[derive(Serialize)]
struct PackageResult {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    teardown_failed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    rendered_refreshed: Option<usize>,
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
/// printed to stdout instead (also when some packages failed). The summary
/// carries each package's real exit code and error kind, and a `teardown_failed`
/// flag when a teardown script failed but the uninstall carried on.
pub fn run_for_packages<F>(
    config: &Config,
    packages: &[String],
    run: &RunOptions,
    labels: &BatchLabels,
    mut apply: F,
) -> Result<()>
where
    F: FnMut(&str) -> PackageOutcome,
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
    // script failure) is preserved. A teardown-only failure stays a warning:
    // the uninstall carried on and stau's documented behavior is to exit 0.
    if !run.json && packages.len() == 1 {
        let outcome = apply(&packages[0]);
        return match outcome.error {
            Some(e) => Err(e),
            None => Ok(()),
        };
    }

    if !run.json {
        println!("{} {} packages...\n", labels.progress, packages.len());
    }

    let mut results = Vec::with_capacity(packages.len());
    for pkg in packages {
        if run.verbose && !run.json {
            println!("--- {} package: {} ---", labels.progress, pkg);
        }

        let outcome = apply(pkg);
        let ok = outcome.error.is_none() && !outcome.teardown_failed;
        if let Some(e) = &outcome.error
            && !run.json
        {
            eprintln!("Failed to {} '{}': {}", labels.action, pkg, e);
        }
        results.push(PackageResult {
            name: pkg.clone(),
            ok,
            error: outcome.error.as_ref().map(|e| e.to_string()),
            exit_code: outcome.error.as_ref().map(|e| e.exit_code()),
            error_kind: outcome.error.as_ref().map(|e| e.kind()),
            teardown_failed: outcome.teardown_failed,
            rendered_refreshed: outcome.rendered_refreshed,
        });

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
