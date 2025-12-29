use crate::error::{Result, StauError};
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

/// Represents a symlink mapping from source to target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkMapping {
    /// The source file in the package directory
    pub source: PathBuf,
    /// The target location where the symlink should be created
    pub target: PathBuf,
}

impl SymlinkMapping {
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        Self { source, target }
    }
}

/// Check if a path is a symlink pointing to the expected target
pub fn is_stau_symlink(path: &Path, expected_target: &Path) -> Result<bool> {
    if !path.exists() && path.symlink_metadata().is_err() {
        return Ok(false);
    }

    match path.symlink_metadata() {
        Ok(metadata) => {
            if metadata.is_symlink() {
                match fs::read_link(path) {
                    Ok(link_target) => Ok(link_target == expected_target),
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false),
    }
}

/// Check if a path or any of its ancestors is a symlink pointing into a dotfiles directory.
/// This helps detect directory symlinks created by other tools (stow, tuckr, etc.)
/// Returns the symlink path if found, None otherwise.
#[allow(dead_code)]
pub fn find_symlink_in_path(path: &Path, stau_dir: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();

    while let Some(parent) = current.parent() {
        if let Ok(metadata) = current.symlink_metadata()
            && metadata.is_symlink()
            && let Ok(link_target) = fs::read_link(&current)
        {
            // Check if the symlink points into the stau_dir
            let resolved = if link_target.is_absolute() {
                link_target
            } else {
                parent.join(&link_target)
            };

            if let Ok(canonical) = resolved.canonicalize()
                && let Ok(stau_canonical) = stau_dir.canonicalize()
                && canonical.starts_with(&stau_canonical)
            {
                return Some(current);
            }
        }
        current = parent.to_path_buf();
    }
    None
}

/// Check if a path is managed by stau (either directly or via a parent directory symlink)
#[allow(dead_code)]
pub fn is_managed_path(path: &Path, expected_target: &Path, stau_dir: &Path) -> Result<bool> {
    // First check if it's a direct symlink
    if is_stau_symlink(path, expected_target)? {
        return Ok(true);
    }

    // Then check if any parent is a symlink pointing into stau_dir
    Ok(find_symlink_in_path(path, stau_dir).is_some())
}

/// Check if a symlink is broken (points to non-existent file)
pub fn is_broken_symlink(path: &Path) -> bool {
    if let Ok(metadata) = path.symlink_metadata()
        && metadata.is_symlink()
    {
        // Check if the target exists
        return !path.exists();
    }
    false
}

/// Remove a directory symlink that points into the dotfiles directory.
/// This helps users migrate from tools that use directory symlinks (stow, tuckr, etc.)
/// Returns Ok(true) if a symlink was removed, Ok(false) if not applicable.
#[allow(dead_code)]
pub fn remove_directory_symlink(path: &Path, stau_dir: &Path, dry_run: bool) -> Result<bool> {
    // Check if path is a symlink
    let metadata = match path.symlink_metadata() {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };

    if !metadata.is_symlink() {
        return Ok(false);
    }

    // Check if the symlink points into stau_dir
    if let Ok(link_target) = fs::read_link(path) {
        let resolved = if link_target.is_absolute() {
            link_target
        } else if let Some(parent) = path.parent() {
            parent.join(&link_target)
        } else {
            link_target
        };

        if let Ok(canonical) = resolved.canonicalize()
            && let Ok(stau_canonical) = stau_dir.canonicalize()
            && canonical.starts_with(&stau_canonical)
        {
            if !dry_run {
                fs::remove_file(path).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        StauError::PermissionDenied(format!(
                            "Cannot remove symlink: {}",
                            path.display()
                        ))
                    } else {
                        StauError::Io(e)
                    }
                })?;
            }
            return Ok(true);
        }
    }

    Ok(false)
}

/// Create a symlink, ensuring parent directories exist
pub fn create_symlink(source: &Path, target: &Path, dry_run: bool) -> Result<()> {
    // Check if target already exists
    if target.exists() || target.symlink_metadata().is_ok() {
        // Check if it's already the correct symlink
        if is_stau_symlink(target, source)? {
            return Ok(()); // Already correct, nothing to do
        }

        return Err(StauError::ConflictingFile(target.to_path_buf()));
    }

    if dry_run {
        return Ok(());
    }

    // Create parent directories if they don't exist
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                StauError::PermissionDenied(format!(
                    "Cannot create directory: {}",
                    parent.display()
                ))
            } else {
                StauError::Io(e)
            }
        })?;
    }

    // Create the symlink
    unix_fs::symlink(source, target).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            StauError::PermissionDenied(format!("Cannot create symlink: {}", target.display()))
        } else {
            StauError::Io(e)
        }
    })?;

    Ok(())
}

/// Remove a symlink if it points to the expected source
pub fn remove_symlink(path: &Path, expected_source: &Path, dry_run: bool) -> Result<bool> {
    if !is_stau_symlink(path, expected_source)? {
        return Ok(false); // Not our symlink, don't remove
    }

    if dry_run {
        return Ok(true);
    }

    fs::remove_file(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            StauError::PermissionDenied(format!("Cannot remove symlink: {}", path.display()))
        } else {
            StauError::Io(e)
        }
    })?;

    Ok(true)
}

/// Copy a file from source to destination
pub fn copy_file(source: &Path, dest: &Path, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }

    if dest.exists() {
        return Err(StauError::ConflictingFile(dest.to_path_buf()));
    }

    // Create parent directories if they don't exist
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                StauError::PermissionDenied(format!(
                    "Cannot create directory: {}",
                    parent.display()
                ))
            } else {
                StauError::Io(e)
            }
        })?;
    }

    fs::copy(source, dest).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            StauError::PermissionDenied(format!("Cannot copy file: {}", dest.display()))
        } else {
            StauError::Io(e)
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_symlink_mapping_creation() {
        let mapping =
            SymlinkMapping::new(PathBuf::from("/source/file"), PathBuf::from("/target/file"));
        assert_eq!(mapping.source, PathBuf::from("/source/file"));
        assert_eq!(mapping.target, PathBuf::from("/target/file"));
    }

    #[test]
    fn test_create_and_check_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source file
        File::create(&source).unwrap();

        // Create symlink
        create_symlink(&source, &target, false).unwrap();

        // Verify it's a stau symlink
        assert!(is_stau_symlink(&target, &source).unwrap());

        // Verify it's not broken
        assert!(!is_broken_symlink(&target));
    }

    #[test]
    fn test_remove_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source file and symlink
        File::create(&source).unwrap();
        create_symlink(&source, &target, false).unwrap();

        // Remove symlink
        let removed = remove_symlink(&target, &source, false).unwrap();
        assert!(removed);
        assert!(!target.exists());
    }

    #[test]
    fn test_conflicting_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create both files
        File::create(&source).unwrap();
        File::create(&target).unwrap();

        // Try to create symlink - should fail
        let result = create_symlink(&source, &target, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StauError::ConflictingFile(_)));
    }

    #[test]
    fn test_broken_symlink_detection() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source file and symlink
        File::create(&source).unwrap();
        unix_fs::symlink(&source, &target).unwrap();

        // Remove source, making symlink broken
        fs::remove_file(&source).unwrap();

        // Verify it's detected as broken
        assert!(is_broken_symlink(&target));
    }

    #[test]
    fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, "test content").unwrap();

        copy_file(&source, &dest, false).unwrap();

        assert_eq!(fs::read_to_string(&dest).unwrap(), "test content");
    }

    #[test]
    fn test_copy_file_with_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("nested/dir/dest.txt");

        fs::write(&source, "test content").unwrap();

        copy_file(&source, &dest, false).unwrap();

        assert_eq!(fs::read_to_string(&dest).unwrap(), "test content");
        assert!(dest.parent().unwrap().exists());
    }

    #[test]
    fn test_copy_file_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, "test content").unwrap();

        copy_file(&source, &dest, true).unwrap();

        assert!(!dest.exists());
    }

    #[test]
    fn test_copy_file_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, "source content").unwrap();
        fs::write(&dest, "dest content").unwrap();

        let result = copy_file(&source, &dest, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StauError::ConflictingFile(_)));
    }

    #[test]
    fn test_remove_symlink_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        File::create(&source).unwrap();
        create_symlink(&source, &target, false).unwrap();

        let removed = remove_symlink(&target, &source, true).unwrap();
        assert!(removed);
        assert!(target.exists()); // Should still exist in dry run
    }

    #[test]
    fn test_remove_wrong_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let other_source = temp_dir.path().join("other.txt");
        let target = temp_dir.path().join("target.txt");

        File::create(&source).unwrap();
        File::create(&other_source).unwrap();
        unix_fs::symlink(&source, &target).unwrap();

        // Try to remove with wrong source
        let removed = remove_symlink(&target, &other_source, false).unwrap();
        assert!(!removed);
        assert!(target.exists()); // Should still exist
    }

    #[test]
    fn test_is_broken_symlink_non_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("file.txt");
        File::create(&file).unwrap();

        assert!(!is_broken_symlink(&file));
    }

    #[test]
    fn test_is_stau_symlink_non_existent() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.txt");
        let source = temp_dir.path().join("source.txt");

        let result = is_stau_symlink(&nonexistent, &source).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_create_symlink_already_correct() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        File::create(&source).unwrap();
        create_symlink(&source, &target, false).unwrap();

        // Creating again with same target should succeed (idempotent)
        create_symlink(&source, &target, false).unwrap();

        assert!(is_stau_symlink(&target, &source).unwrap());
    }

    #[test]
    fn test_symlink_mapping_equality() {
        let mapping1 =
            SymlinkMapping::new(PathBuf::from("/source/file"), PathBuf::from("/target/file"));
        let mapping2 =
            SymlinkMapping::new(PathBuf::from("/source/file"), PathBuf::from("/target/file"));
        let mapping3 = SymlinkMapping::new(
            PathBuf::from("/source/other"),
            PathBuf::from("/target/other"),
        );

        assert_eq!(mapping1, mapping2);
        assert_ne!(mapping1, mapping3);
    }

    #[test]
    fn test_find_symlink_in_path() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a package with nested structure
        let package_config = stau_dir.join("vim").join(".config").join("nvim");
        fs::create_dir_all(&package_config).unwrap();
        File::create(package_config.join("init.lua")).unwrap();

        // Create a directory symlink (like stow would)
        let target_config = target_dir.join(".config");
        unix_fs::symlink(stau_dir.join("vim").join(".config"), &target_config).unwrap();

        // Check that we can find the symlink from a nested path
        let nested_path = target_config.join("nvim").join("init.lua");
        let found = find_symlink_in_path(&nested_path, &stau_dir);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), target_config);
    }

    #[test]
    fn test_find_symlink_in_path_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a real directory (not a symlink)
        let target_config = target_dir.join(".config").join("nvim");
        fs::create_dir_all(&target_config).unwrap();
        File::create(target_config.join("init.lua")).unwrap();

        // No symlink should be found
        let found = find_symlink_in_path(&target_config.join("init.lua"), &stau_dir);
        assert!(found.is_none());
    }

    #[test]
    fn test_remove_directory_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a package directory
        let package_config = stau_dir.join("vim").join(".config");
        fs::create_dir_all(&package_config).unwrap();

        // Create a directory symlink
        let target_config = target_dir.join(".config");
        unix_fs::symlink(&package_config, &target_config).unwrap();

        // Remove the directory symlink
        let removed = remove_directory_symlink(&target_config, &stau_dir, false).unwrap();
        assert!(removed);
        assert!(!target_config.exists());
    }

    #[test]
    fn test_remove_directory_symlink_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a package directory
        let package_config = stau_dir.join("vim").join(".config");
        fs::create_dir_all(&package_config).unwrap();

        // Create a directory symlink
        let target_config = target_dir.join(".config");
        unix_fs::symlink(&package_config, &target_config).unwrap();

        // Dry run should not remove
        let removed = remove_directory_symlink(&target_config, &stau_dir, true).unwrap();
        assert!(removed);
        assert!(target_config.symlink_metadata().is_ok()); // Still exists
    }

    #[test]
    fn test_remove_directory_symlink_not_in_stau_dir() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let other_dir = temp_dir.path().join("other");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&other_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a symlink pointing elsewhere (not in stau_dir)
        let target_config = target_dir.join(".config");
        unix_fs::symlink(&other_dir, &target_config).unwrap();

        // Should not remove symlinks pointing outside stau_dir
        let removed = remove_directory_symlink(&target_config, &stau_dir, false).unwrap();
        assert!(!removed);
        assert!(target_config.symlink_metadata().is_ok()); // Still exists
    }

    #[test]
    fn test_is_managed_path() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("home");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create source file
        let source = stau_dir.join("vim").join(".vimrc");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        File::create(&source).unwrap();

        // Create direct symlink
        let target = target_dir.join(".vimrc");
        unix_fs::symlink(&source, &target).unwrap();

        // Should be detected as managed
        assert!(is_managed_path(&target, &source, &stau_dir).unwrap());
    }
}
