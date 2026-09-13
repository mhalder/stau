use crate::error::{Result, StauError, map_io_error};
use crate::symlink::{self, SymlinkMapping};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Walks a package directory and generates symlink mappings for all files.
///
/// Returns a list of `SymlinkMapping` structs representing the source files in the package
/// and their corresponding target locations. Setup/teardown scripts and version control
/// files at the package root are excluded.
pub fn discover_package_files(
    package_dir: &Path,
    target_dir: &Path,
) -> Result<Vec<SymlinkMapping>> {
    if !package_dir.exists() {
        return Err(StauError::PackageNotFound(
            package_dir.display().to_string(),
        ));
    }

    if !package_dir.is_dir() {
        return Err(StauError::InvalidPath(package_dir.to_path_buf()));
    }

    let mut mappings = Vec::new();
    walk_directory(
        package_dir,
        package_dir,
        target_dir,
        &mut mappings,
        &mut Vec::new(),
    )?;
    // read_dir order is filesystem-dependent; sort for deterministic output
    mappings.sort_by(|a, b| a.target.cmp(&b.target));
    Ok(mappings)
}

/// Finds broken symlinks in the target directory that point into the package.
///
/// These are left behind when files are removed from a package after it was
/// installed. Only target directories mirroring a directory that still exists in
/// the package are scanned, so links below a directory removed from the package
/// are not found.
pub fn discover_broken_symlinks(package_dir: &Path, target_dir: &Path) -> Result<Vec<PathBuf>> {
    if !package_dir.is_dir() {
        return Err(StauError::PackageNotFound(
            package_dir.display().to_string(),
        ));
    }

    let mut dirs = vec![PathBuf::new()];
    walk_directory(
        package_dir,
        package_dir,
        target_dir,
        &mut Vec::new(),
        &mut dirs,
    )?;

    let mut broken = Vec::new();
    for rel_dir in dirs {
        let dir = target_dir.join(rel_dir);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                continue;
            }
            Err(e) => {
                return Err(map_io_error(
                    e,
                    format!("Cannot read directory: {}", dir.display()),
                ));
            }
        };

        for entry in entries {
            let path = entry.map_err(StauError::Io)?.path();
            if symlink::is_broken_package_symlink(&path, package_dir) {
                broken.push(path);
            }
        }
    }

    broken.sort();
    Ok(broken)
}

/// Recursively walk a directory, collecting symlink mappings for files and the
/// paths of subdirectories relative to the package root
fn walk_directory(
    base_dir: &Path,
    current_dir: &Path,
    target_dir: &Path,
    mappings: &mut Vec<SymlinkMapping>,
    dirs: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = fs::read_dir(current_dir).map_err(|e| {
        map_io_error(
            e,
            format!("Cannot read directory: {}", current_dir.display()),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(StauError::Io)?;
        let path = entry.path();
        let file_name = entry.file_name();

        // Skip package scripts and version control files in root of package
        let file_name_str = file_name.to_string_lossy();
        if current_dir == base_dir
            && matches!(
                file_name_str.as_ref(),
                "setup.sh"
                    | "teardown.sh"
                    | ".git"
                    | ".gitignore"
                    | ".gitattributes"
                    | ".gitmodules"
            )
        {
            continue;
        }

        let metadata = entry.metadata().map_err(StauError::Io)?;

        // Skip symlinks and other special files
        if !metadata.is_dir() && !metadata.is_file() {
            continue;
        }

        // Calculate relative path from package base
        let rel_path = path
            .strip_prefix(base_dir)
            .map_err(|_| StauError::InvalidPath(path.clone()))?
            .to_path_buf();

        if metadata.is_dir() {
            // Recursively walk subdirectories
            walk_directory(base_dir, &path, target_dir, mappings, dirs)?;
            dirs.push(rel_path);
        } else {
            // Target path is target_dir + relative path
            let target_path = target_dir.join(&rel_path);
            mappings.push(SymlinkMapping::new(path, target_path));
        }
    }

    Ok(())
}

/// Lists all packages in the stau directory.
///
/// Returns a sorted list of package names (directory names) in STAU_DIR.
/// Hidden directories (starting with `.`) are excluded.
pub fn list_packages(stau_dir: &Path) -> Result<Vec<String>> {
    if !stau_dir.exists() {
        return Err(StauError::StauDirNotFound(stau_dir.to_path_buf()));
    }

    let entries = fs::read_dir(stau_dir)
        .map_err(|e| map_io_error(e, format!("Cannot read directory: {}", stau_dir.display())))?;

    let mut packages = Vec::new();
    for entry in entries {
        let entry = entry.map_err(StauError::Io)?;
        let path = entry.path();

        // Only include directories, skip hidden directories
        if path.is_dir()
            && let Some(name) = path.file_name()
            && !name.to_string_lossy().starts_with('.')
        {
            packages.push(name.to_string_lossy().to_string());
        }
    }

    packages.sort();
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_discover_simple_package() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("test_package");
        let target_dir = temp_dir.path().join("target");

        // Create package structure
        fs::create_dir(&package_dir).unwrap();
        File::create(package_dir.join(".bashrc")).unwrap();
        File::create(package_dir.join(".vimrc")).unwrap();

        let mappings = discover_package_files(&package_dir, &target_dir).unwrap();

        assert_eq!(mappings.len(), 2);
        assert!(
            mappings
                .iter()
                .any(|m| m.source.ends_with(".bashrc") && m.target.ends_with(".bashrc"))
        );
        assert!(
            mappings
                .iter()
                .any(|m| m.source.ends_with(".vimrc") && m.target.ends_with(".vimrc"))
        );
    }

    #[test]
    fn test_discover_nested_package() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("test_package");
        let target_dir = temp_dir.path().join("target");

        // Create nested structure
        fs::create_dir(&package_dir).unwrap();
        fs::create_dir_all(package_dir.join(".config/nvim")).unwrap();
        File::create(package_dir.join(".config/nvim/init.lua")).unwrap();
        File::create(package_dir.join(".bashrc")).unwrap();

        let mappings = discover_package_files(&package_dir, &target_dir).unwrap();

        assert_eq!(mappings.len(), 2);
        assert!(
            mappings
                .iter()
                .any(|m| m.source.ends_with("init.lua")
                    && m.target.ends_with(".config/nvim/init.lua"))
        );
    }

    #[test]
    fn test_skip_setup_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("test_package");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&package_dir).unwrap();
        File::create(package_dir.join("setup.sh")).unwrap();
        File::create(package_dir.join("teardown.sh")).unwrap();
        File::create(package_dir.join(".bashrc")).unwrap();

        let mappings = discover_package_files(&package_dir, &target_dir).unwrap();

        // Should only find .bashrc, not the scripts
        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].source.ends_with(".bashrc"));
    }

    #[test]
    fn test_skip_hidden_root_files() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("test_package");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&package_dir).unwrap();
        File::create(package_dir.join(".git")).unwrap();
        File::create(package_dir.join(".gitignore")).unwrap();
        File::create(package_dir.join(".bashrc")).unwrap();

        let mappings = discover_package_files(&package_dir, &target_dir).unwrap();

        // Should skip .git and .gitignore at root, but include .bashrc (it's a config file)
        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].source.ends_with(".bashrc"));
    }

    #[test]
    fn test_list_packages() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path();

        // Create some package directories
        fs::create_dir(stau_dir.join("vim")).unwrap();
        fs::create_dir(stau_dir.join("tmux")).unwrap();
        fs::create_dir(stau_dir.join("git")).unwrap();
        fs::create_dir(stau_dir.join(".hidden")).unwrap();

        let packages = list_packages(stau_dir).unwrap();

        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"vim".to_string()));
        assert!(packages.contains(&"tmux".to_string()));
        assert!(packages.contains(&"git".to_string()));
        assert!(!packages.contains(&".hidden".to_string()));
    }

    #[test]
    fn test_nonexistent_package() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("nonexistent");
        let target_dir = temp_dir.path().join("target");

        let result = discover_package_files(&package_dir, &target_dir);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StauError::PackageNotFound(_)));
    }

    #[test]
    fn test_discover_broken_symlinks() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("pkg");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir_all(package_dir.join(".config/app")).unwrap();
        fs::create_dir_all(target_dir.join(".config/app")).unwrap();
        File::create(package_dir.join(".config/app/kept")).unwrap();

        // Stale links to files removed from the package, at root and nested
        unix_fs::symlink(
            package_dir.join(".removedrc"),
            target_dir.join(".removedrc"),
        )
        .unwrap();
        unix_fs::symlink(
            package_dir.join(".config/app/removed"),
            target_dir.join(".config/app/removed"),
        )
        .unwrap();
        // A valid link and an unrelated broken link are not reported
        unix_fs::symlink(
            package_dir.join(".config/app/kept"),
            target_dir.join(".config/app/kept"),
        )
        .unwrap();
        unix_fs::symlink("/nonexistent", target_dir.join(".config/app/other")).unwrap();

        let broken = discover_broken_symlinks(&package_dir, &target_dir).unwrap();

        assert_eq!(
            broken,
            vec![
                target_dir.join(".config/app/removed"),
                target_dir.join(".removedrc"),
            ]
        );
    }

    #[test]
    fn test_discover_broken_symlinks_missing_target() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("pkg");
        fs::create_dir_all(package_dir.join(".config")).unwrap();

        let broken =
            discover_broken_symlinks(&package_dir, &temp_dir.path().join("missing")).unwrap();
        assert!(broken.is_empty());
    }

    #[test]
    fn test_nested_script_names_are_linked() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("test_package");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(package_dir.join(".local/bin")).unwrap();
        fs::create_dir_all(package_dir.join("sub")).unwrap();
        File::create(package_dir.join("setup.sh")).unwrap();
        File::create(package_dir.join(".local/bin/setup.sh")).unwrap();
        File::create(package_dir.join(".local/bin/teardown.sh")).unwrap();
        File::create(package_dir.join("sub/.gitignore")).unwrap();

        let mappings = discover_package_files(&package_dir, &target_dir).unwrap();

        assert_eq!(mappings.len(), 3);
        assert!(
            !mappings
                .iter()
                .any(|m| m.target == target_dir.join("setup.sh"))
        );
        assert!(
            mappings
                .iter()
                .any(|m| m.target == target_dir.join(".local/bin/setup.sh"))
        );
        assert!(
            mappings
                .iter()
                .any(|m| m.target == target_dir.join("sub/.gitignore"))
        );
    }
}
