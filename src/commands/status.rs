use crate::config::Config;
use crate::error::{self, Result};
use crate::package;
use crate::symlink::{self, MappingKind};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The per-file state stau can observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Installed,
    NotInstalled,
    Conflict,
    Broken,
    Restored,
    Drift,
}

impl FileState {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileState::Installed => "installed",
            FileState::NotInstalled => "not_installed",
            FileState::Conflict => "conflict",
            FileState::Broken => "broken",
            FileState::Restored => "restored",
            FileState::Drift => "drift",
        }
    }
}

/// Computes the state of one file mapping against the target directory.
///
/// For a `Symlink` mapping, a target that is a regular file whose contents
/// equal the package source is `Restored` — stau's copy-back contract after
/// uninstall. For a `Render` mapping, a matching regular file is `Installed`, a
/// differing regular file is `Drift`, and a symlink or directory is `Conflict`.
/// A read error on either file is returned as an I/O error (used by `verify`,
/// which must fail on I/O).
pub fn compute_file_state(
    mapping: &symlink::SymlinkMapping,
    broken: &HashSet<PathBuf>,
) -> Result<FileState> {
    compute_file_state_inner(mapping, broken, false)
}

/// Like [`compute_file_state`], but an unreadable target or source file is
/// reported as `Conflict` instead of aborting the whole status envelope.
pub fn compute_file_state_lenient(
    mapping: &symlink::SymlinkMapping,
    broken: &HashSet<PathBuf>,
) -> FileState {
    compute_file_state_inner(mapping, broken, true).expect("lenient state computation never fails")
}

fn compute_file_state_inner(
    mapping: &symlink::SymlinkMapping,
    broken: &HashSet<PathBuf>,
    lenient: bool,
) -> Result<FileState> {
    match mapping.kind {
        MappingKind::Symlink => {
            if broken.contains(&mapping.target) {
                return Ok(FileState::Broken);
            }

            if symlink::is_stau_symlink(&mapping.target, &mapping.source)? {
                return Ok(FileState::Installed);
            }

            match mapping.target.symlink_metadata() {
                Ok(metadata) if metadata.is_symlink() => Ok(FileState::Conflict),
                Ok(metadata) if metadata.is_file() => {
                    match symlink::same_contents(&mapping.source, &mapping.target) {
                        Ok(true) => Ok(FileState::Restored),
                        Ok(false) => Ok(FileState::Conflict),
                        Err(_) if lenient => Ok(FileState::Conflict),
                        Err(e) => Err(e),
                    }
                }
                Ok(_) => Ok(FileState::Conflict),
                Err(_) => Ok(FileState::NotInstalled),
            }
        }
        MappingKind::Render => {
            let content = mapping
                .rendered
                .as_deref()
                .expect("render mapping carries content");
            match mapping.target.symlink_metadata() {
                Ok(metadata) if metadata.is_symlink() => Ok(FileState::Conflict),
                Ok(metadata) if metadata.is_file() => {
                    match symlink::same_bytes(&mapping.target, content) {
                        Ok(true) => Ok(FileState::Installed),
                        Ok(false) => Ok(FileState::Drift),
                        Err(_) if lenient => Ok(FileState::Conflict),
                        Err(e) => Err(e),
                    }
                }
                Ok(_) => Ok(FileState::Conflict),
                Err(_) => Ok(FileState::NotInstalled),
            }
        }
    }
}

#[derive(Serialize)]
struct StatusFile {
    rel: String,
    target: String,
    state: String,
}

#[derive(Serialize)]
struct StatusCounts {
    installed: usize,
    not_installed: usize,
    conflict: usize,
    broken: usize,
    drift: usize,
}

#[derive(Serialize)]
struct StatusPackage {
    name: String,
    state: String,
    setup_script: bool,
    teardown_script: bool,
    counts: StatusCounts,
    files: Vec<StatusFile>,
}

#[derive(Serialize)]
struct StatusEnvelope {
    stau_dir: String,
    target: String,
    packages: Vec<StatusPackage>,
}

/// Entry point for `stau status`: human text for one or more packages, or a
/// JSON envelope for one or more packages.
pub fn run_status(
    config: &Config,
    packages: Vec<String>,
    target: Option<PathBuf>,
    json: bool,
    data: &serde_json::Value,
) -> Result<()> {
    if json {
        show_status_json(config, &packages, target, data)
    } else {
        for package in &packages {
            Config::validate_package_name(package)?;
            show_status(config, package, target.clone(), data)?;
        }
        Ok(())
    }
}

/// Shows detailed status for a specific package including symlink states and script availability.
pub fn show_status(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    data: &serde_json::Value,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    println!("Status for package '{}':\n", package);
    println!("  Package directory: {}", package_dir.display());
    println!("  Target directory:  {}", target_dir.display());

    // Check for setup/teardown scripts
    if let Some(setup) = config.get_setup_script(package) {
        println!("  Setup script:      {} (exists)", setup.display());
    } else {
        println!("  Setup script:      (none)");
    }

    if let Some(teardown) = config.get_teardown_script(package) {
        println!("  Teardown script:   {} (exists)", teardown.display());
    } else {
        println!("  Teardown script:   (none)");
    }

    // Get all mappings
    let mappings = package::discover_package_files(&package_dir, &target_dir, data)?;
    let broken = package::discover_broken_symlinks(&package_dir, &target_dir)?;
    let broken_set: HashSet<PathBuf> = broken.iter().cloned().collect();

    if mappings.is_empty() && broken.is_empty() {
        if config.get_setup_script(package).is_some()
            || config.get_teardown_script(package).is_some()
        {
            println!("\nNo files in package (hook-only).");
        } else {
            println!("\nNo files in package.");
        }
        return Ok(());
    }

    println!("\nFiles ({} total):", mappings.len());

    let mut installed = 0;
    let mut not_installed = 0;
    let mut drift = 0;

    for mapping in &mappings {
        let state = compute_file_state_lenient(mapping, &broken_set);
        let (label, count_not_installed) = match state {
            FileState::Installed => {
                installed += 1;
                ("[installed]", false)
            }
            FileState::NotInstalled => ("[not installed]", true),
            FileState::Conflict => ("[conflict]", true),
            FileState::Broken => ("[BROKEN]", false),
            FileState::Restored => ("[restored]", true),
            FileState::Drift => {
                drift += 1;
                ("[drift]", false)
            }
        };
        if count_not_installed {
            not_installed += 1;
        }

        println!("  {:<20} {}", label, mapping.target.display());
    }

    // Broken symlinks to files that were removed from the package
    for path in broken
        .iter()
        .filter(|path| !mappings.iter().any(|mapping| mapping.target == **path))
    {
        println!("  {:<20} {}", "[BROKEN]", path.display());
    }

    let mut summary = format!(
        "Summary: {} installed, {} not installed",
        installed, not_installed
    );
    if drift > 0 {
        summary.push_str(&format!(", {} drift", drift));
    }
    summary.push_str(&format!(", {} broken", broken.len()));
    println!();
    println!("{summary}");

    Ok(())
}

/// Prints the machine-readable status envelope for the given packages, or every
/// package when none are named.
pub fn show_status_json(
    config: &Config,
    packages: &[String],
    target: Option<PathBuf>,
    data: &serde_json::Value,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let names: Vec<String> = if packages.is_empty() {
        package::list_packages(&config.stau_dir)?
    } else {
        for package in packages {
            Config::validate_package_name(package)?;
        }
        packages.to_vec()
    };

    let mut result = Vec::with_capacity(names.len());
    for name in &names {
        result.push(collect_package_status(config, name, &target_dir, data)?);
    }

    let envelope = StatusEnvelope {
        stau_dir: config.stau_dir.display().to_string(),
        target: target_dir.display().to_string(),
        packages: result,
    };

    println!(
        "{}",
        serde_json::to_string(&envelope).expect("status envelope is always serializable")
    );
    Ok(())
}

/// Builds the JSON status for a single package.
fn collect_package_status(
    config: &Config,
    package: &str,
    target_dir: &Path,
    data: &serde_json::Value,
) -> Result<StatusPackage> {
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    let mappings = package::discover_package_files(&package_dir, target_dir, data)?;
    let broken = package::discover_broken_symlinks(&package_dir, target_dir)?;
    let broken_set: HashSet<PathBuf> = broken.iter().cloned().collect();

    let mut files = Vec::new();
    let mut counts = StatusCounts {
        installed: 0,
        not_installed: 0,
        conflict: 0,
        broken: 0,
        drift: 0,
    };

    for mapping in &mappings {
        let state = compute_file_state_lenient(mapping, &broken_set);
        match state {
            FileState::Installed => counts.installed += 1,
            FileState::NotInstalled | FileState::Restored => counts.not_installed += 1,
            FileState::Conflict => counts.conflict += 1,
            FileState::Broken => counts.broken += 1,
            FileState::Drift => counts.drift += 1,
        }
        files.push(StatusFile {
            rel: rel_to_target(&mapping.target, target_dir),
            target: mapping.target.display().to_string(),
            state: state.as_str().to_string(),
        });
    }

    // Broken symlinks left by files removed from the package entirely
    for path in &broken {
        if mappings.iter().any(|mapping| mapping.target == *path) {
            continue;
        }
        counts.broken += 1;
        files.push(StatusFile {
            rel: rel_to_target(path, target_dir),
            target: path.display().to_string(),
            state: FileState::Broken.as_str().to_string(),
        });
    }

    let total = mappings.len();
    let setup_script = config.get_setup_script(package).is_some();
    let teardown_script = config.get_teardown_script(package).is_some();
    let state = if counts.conflict > 0 {
        "conflict"
    } else if total == 0 && (setup_script || teardown_script) {
        // A package with no files at all has nothing to link: what installing
        // it means is running its hooks, which stau does not record. Neither
        // "installed" nor "not_installed" is true, so say what is.
        "hook_only"
    } else if total == 0 || counts.installed == 0 {
        "not_installed"
    } else if counts.installed == total {
        "installed"
    } else {
        "partial"
    };

    Ok(StatusPackage {
        name: package.to_string(),
        state: state.to_string(),
        setup_script,
        teardown_script,
        counts,
        files,
    })
}

/// Returns a path relative to `target_dir`, falling back to the absolute path.
pub fn rel_to_target(path: &Path, target_dir: &Path) -> String {
    path.strip_prefix(target_dir)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use tempfile::TempDir;

    fn render_mapping(target: std::path::PathBuf, content: &str) -> symlink::SymlinkMapping {
        symlink::SymlinkMapping::render(
            std::path::PathBuf::from("/pkg/src.tmpl"),
            target,
            content.to_string(),
        )
    }

    #[test]
    fn test_render_installed_when_matching() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("file");
        fs::write(&target, "hello").unwrap();

        let mapping = render_mapping(target, "hello");
        assert_eq!(
            compute_file_state(&mapping, &HashSet::new()).unwrap(),
            FileState::Installed
        );
    }

    #[test]
    fn test_render_drift_when_modified() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("file");
        fs::write(&target, "modified").unwrap();

        let mapping = render_mapping(target, "hello");
        assert_eq!(
            compute_file_state(&mapping, &HashSet::new()).unwrap(),
            FileState::Drift
        );
    }

    #[test]
    fn test_render_not_installed_when_absent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("missing");

        let mapping = render_mapping(target, "hello");
        assert_eq!(
            compute_file_state(&mapping, &HashSet::new()).unwrap(),
            FileState::NotInstalled
        );
    }

    #[test]
    fn test_render_conflict_when_symlink() {
        use std::os::unix::fs as unix_fs;

        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("file");
        unix_fs::symlink("/somewhere", &target).unwrap();

        let mapping = render_mapping(target, "hello");
        assert_eq!(
            compute_file_state(&mapping, &HashSet::new()).unwrap(),
            FileState::Conflict
        );
    }
}
