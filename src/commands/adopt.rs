use crate::config::Config;
use crate::error::{self, Result, map_io_error};
use crate::symlink;
use std::fs;
use std::path::{Path, PathBuf};

/// Adopts existing files into a package by moving them to the package directory and creating symlinks.
///
/// Every file is checked before any is moved, so a conflict does not leave the
/// package partially adopted.
pub fn adopt_files(
    config: &Config,
    package: &str,
    files: &[PathBuf],
    target: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    println!(
        "Adopting {} file(s) into package '{}':",
        files.len(),
        package
    );

    let mut adoptions: Vec<(PathBuf, PathBuf)> = Vec::new();
    for file_path in files {
        if let Some(adoption) = adopt_destination(file_path, &target_dir, &package_dir)?
            && !adoptions.iter().any(|(_, dest)| *dest == adoption.1)
        {
            adoptions.push(adoption);
        }
    }

    if adoptions.is_empty() {
        println!("No files adopted into '{}'", package);
        return Ok(());
    }

    ensure_package_dir(&package_dir, dry_run, verbose)?;

    for (file_path, dest) in &adoptions {
        if verbose || dry_run {
            println!("  {} -> {}", file_path.display(), dest.display());
        }

        if !dry_run {
            move_file_into_package(file_path, dest)?;
        }
    }

    if !dry_run {
        println!(
            "Successfully adopted {} file(s) into '{}'",
            adoptions.len(),
            package
        );
    }

    Ok(())
}

fn ensure_package_dir(package_dir: &Path, dry_run: bool, verbose: bool) -> Result<()> {
    if package_dir.exists() {
        return Ok(());
    }

    if verbose || dry_run {
        println!("Creating package directory: {}", package_dir.display());
    }

    if !dry_run {
        fs::create_dir_all(package_dir).map_err(|e| {
            map_io_error(
                e,
                format!("Cannot create package directory: {}", package_dir.display()),
            )
        })?;
    }

    Ok(())
}

/// Resolves where a file will be adopted to, returning `(absolute file path, destination)`.
///
/// Relative paths are resolved against the current directory. Missing files, files outside
/// the target directory, directories, and symlinks are skipped with a warning.
fn adopt_destination(
    file_path: &Path,
    target_dir: &Path,
    package_dir: &Path,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let file_path = std::path::absolute(file_path).map_err(error::StauError::Io)?;
    let target_dir = std::path::absolute(target_dir).map_err(error::StauError::Io)?;

    let Ok(metadata) = file_path.symlink_metadata() else {
        eprintln!("Warning: File does not exist: {}", file_path.display());
        return Ok(None);
    };

    if !metadata.is_file() {
        let kind = if metadata.is_dir() {
            "a directory"
        } else if metadata.is_symlink() {
            "a symlink"
        } else {
            "a special file"
        };
        eprintln!(
            "Warning: Skipping {} ({}); only regular files can be adopted",
            file_path.display(),
            kind
        );
        return Ok(None);
    }

    let Ok(rel_path) = file_path.strip_prefix(&target_dir) else {
        eprintln!(
            "Warning: File {} is not in target directory {}",
            file_path.display(),
            target_dir.display()
        );
        return Ok(None);
    };

    let dest = package_dir.join(rel_path);
    if dest.symlink_metadata().is_ok() {
        return Err(error::StauError::ConflictingFile(dest));
    }

    Ok(Some((file_path, dest)))
}

fn move_file_into_package(file_path: &Path, dest: &Path) -> Result<()> {
    symlink::move_file(file_path, dest)?;
    symlink::create_symlink(dest, file_path, false)
}
