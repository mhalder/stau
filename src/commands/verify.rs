use super::status::{compute_file_state, rel_to_target};
use crate::cli::Expect;
use crate::config::Config;
use crate::error::{self, Result, StauError};
use crate::package;
use crate::template;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Serialize)]
struct VerifyMismatch {
    rel: String,
    expected: String,
    actual: String,
}

#[derive(Serialize)]
struct VerifyPackage {
    name: String,
    ok: bool,
    mismatches: Vec<VerifyMismatch>,
}

#[derive(Serialize)]
struct VerifyEnvelope {
    ok: bool,
    packages: Vec<VerifyPackage>,
}

impl Expect {
    /// The file state a package's files must be in to satisfy this expectation.
    fn expected_state(&self) -> &'static str {
        match self {
            Expect::Installed => "installed",
            Expect::Uninstalled => "restored",
        }
    }
}

/// Verifies every file of every named package is in the expected state.
///
/// Exits 0 when all files match, 5 when any mismatch, 1 when a package does not
/// exist, and 3 on I/O errors.
pub fn run_verify(
    config: &Config,
    packages: &[String],
    expect: Expect,
    target: Option<PathBuf>,
    json: bool,
    data: &serde_json::Value,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let expected = expect.expected_state();

    let mut results = Vec::with_capacity(packages.len());
    for package in packages {
        Config::validate_package_name(package)?;

        if !config.package_exists(package) {
            return Err(error::StauError::PackageNotFound(package.to_string()));
        }

        let package_dir = config.get_package_dir(package);
        let mut mappings = package::discover_package_files(&package_dir, &target_dir)?;
        template::render_mappings(&mut mappings, data);
        let broken = package::discover_broken_symlinks(&package_dir, &target_dir)?;
        let broken_set: HashSet<PathBuf> = broken.iter().cloned().collect();

        let mut mismatches = Vec::new();
        for mapping in &mappings {
            let actual = compute_file_state(mapping, &broken_set)?;
            if actual.as_str() != expected {
                mismatches.push(VerifyMismatch {
                    rel: rel_to_target(&mapping.target, &target_dir),
                    expected: expected.to_string(),
                    actual: actual.as_str().to_string(),
                });
            }
        }

        results.push(VerifyPackage {
            name: package.to_string(),
            ok: mismatches.is_empty(),
            mismatches,
        });
    }

    let ok = results.iter().all(|package| package.ok);
    let mismatch_count: usize = results.iter().map(|p| p.mismatches.len()).sum();

    if json {
        println!(
            "{}",
            serde_json::to_string(&VerifyEnvelope {
                ok,
                packages: results,
            })
            .expect("verify envelope is always serializable")
        );
    } else {
        for package in &results {
            for mismatch in &package.mismatches {
                println!(
                    "{}: expected {}, found {}",
                    mismatch.rel, mismatch.expected, mismatch.actual
                );
            }
        }
    }

    if ok {
        Ok(())
    } else {
        Err(StauError::VerifyMismatch(format!(
            "{mismatch_count} file(s) not in the expected state"
        )))
    }
}
