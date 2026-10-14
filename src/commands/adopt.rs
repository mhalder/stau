use crate::config::Config;
use crate::error::{self, Result, map_io_error};
use crate::symlink;
use crate::template;
use std::fs;
use std::path::{Path, PathBuf};

/// Options for an adopt run that go beyond the shared target/dry-run/verbose.
pub struct AdoptOptions<'a> {
    /// Store `<rel>.tmpl` and render it back to the target.
    pub template: bool,
    /// The merged template data used when rendering an adopted template.
    pub data: &'a serde_json::Value,
}

/// Adopts existing files into a package by moving them to the package directory and creating symlinks.
///
/// Every file is checked before any is moved, so a conflict does not leave the
/// package partially adopted. With `options.template`, the destination inside
/// the package becomes `<rel>.tmpl`; each template is rendered exactly once,
/// up front, and the captured content is written back to the original target
/// after the move (so a concurrent write between check and move cannot strand
/// the file in the package).
pub fn adopt_files(
    config: &Config,
    package: &str,
    files: &[PathBuf],
    target: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
    options: AdoptOptions<'_>,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    println!(
        "Adopting {} file(s) into package '{}':",
        files.len(),
        package
    );

    let mut adoptions: Vec<(PathBuf, PathBuf, Option<String>)> = Vec::new();
    for file_path in files {
        if let Some(adoption) =
            adopt_destination(file_path, &target_dir, &package_dir, options.template)?
            && !adoptions.iter().any(|(_, dest, _)| *dest == adoption.1)
        {
            adoptions.push((adoption.0, adoption.1, None));
        }
    }

    if adoptions.is_empty() {
        println!("No files adopted into '{}'", package);
        return Ok(());
    }

    // With --template, render every candidate exactly once, up front, and keep
    // the rendered bytes. A file with literal `{{ }}` (GitHub Actions, helm, go
    // templates) must fail here, before anything is moved into the package,
    // and the captured content must be written to the target after the move
    // instead of re-rendering the moved template. This keeps multi-file adopt
    // all-or-nothing and closes the check-then-move render window.
    if options.template {
        for (file_path, _, rendered) in &mut adoptions {
            *rendered = Some(template::render(file_path, options.data)?);
        }
    }

    ensure_package_dir(&package_dir, dry_run, verbose)?;

    for (file_path, dest, rendered) in &adoptions {
        if verbose || dry_run {
            println!("  {} -> {}", file_path.display(), dest.display());
        }

        if !dry_run {
            move_file_into_package(file_path, dest, rendered.as_deref())?;
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
/// the target directory, directories, and symlinks are skipped with a warning. With
/// `template`, the destination gets a `.tmpl` suffix.
fn adopt_destination(
    file_path: &Path,
    target_dir: &Path,
    package_dir: &Path,
    template: bool,
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

    let dest = if template {
        let mut os = rel_path.as_os_str().to_os_string();
        os.push(".tmpl");
        package_dir.join(PathBuf::from(os))
    } else {
        package_dir.join(rel_path)
    };
    if dest.symlink_metadata().is_ok() {
        return Err(error::StauError::ConflictingFile(dest));
    }

    Ok(Some((file_path, dest)))
}

fn move_file_into_package(file_path: &Path, dest: &Path, rendered: Option<&str>) -> Result<()> {
    symlink::move_file(file_path, dest)?;
    match rendered {
        Some(content) => symlink::write_rendered(dest, file_path, content, false)?,
        None => symlink::create_symlink(dest, file_path, false)?,
    }
    Ok(())
}
