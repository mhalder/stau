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

/// Create a symlink, ensuring parent directories exist
pub fn create_symlink(source: &Path, target: &Path, dry_run: bool) -> Result<()> {
    create_symlink_with_force(source, target, dry_run, false)
}

/// Create a symlink with optional force flag to overwrite existing files
pub fn create_symlink_with_force(
    source: &Path,
    target: &Path,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    // Check if target already exists
    if target.exists() || target.symlink_metadata().is_ok() {
        // Check if it's already the correct symlink
        if is_stau_symlink(target, source)? {
            return Ok(()); // Already correct, nothing to do
        }

        if !force {
            return Err(StauError::ConflictingFile(target.to_path_buf()));
        }

        // Force enabled: remove the existing file/symlink
        if !dry_run {
            let metadata = target.symlink_metadata()?;
            if metadata.is_symlink() {
                fs::remove_file(target).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        StauError::PermissionDenied(format!(
                            "Cannot remove existing symlink: {}",
                            target.display()
                        ))
                    } else {
                        StauError::Io(e)
                    }
                })?;
            } else if metadata.is_file() {
                fs::remove_file(target).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        StauError::PermissionDenied(format!(
                            "Cannot remove existing file: {}",
                            target.display()
                        ))
                    } else {
                        StauError::Io(e)
                    }
                })?;
            } else if metadata.is_dir() {
                fs::remove_dir_all(target).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        StauError::PermissionDenied(format!(
                            "Cannot remove existing directory: {}",
                            target.display()
                        ))
                    } else {
                        StauError::Io(e)
                    }
                })?;
            }
        }
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
    fn test_force_overwrite_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source and target files
        File::create(&source).unwrap();
        fs::write(&target, "existing content").unwrap();

        // Without force, should fail
        let result = create_symlink_with_force(&source, &target, false, false);
        assert!(result.is_err());

        // With force, should succeed
        create_symlink_with_force(&source, &target, false, true).unwrap();

        // Verify the symlink was created
        assert!(is_stau_symlink(&target, &source).unwrap());
    }

    #[test]
    fn test_force_overwrite_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target_dir");

        // Create source file and target directory with a file inside
        File::create(&source).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        // Without force, should fail
        let result = create_symlink_with_force(&source, &target, false, false);
        assert!(result.is_err());

        // With force, should succeed and remove the entire directory
        create_symlink_with_force(&source, &target, false, true).unwrap();

        // Verify the symlink was created
        assert!(is_stau_symlink(&target, &source).unwrap());
    }

    #[test]
    fn test_force_overwrite_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let old_source = temp_dir.path().join("old_source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source files
        File::create(&source).unwrap();
        File::create(&old_source).unwrap();

        // Create symlink pointing to old source
        unix_fs::symlink(&old_source, &target).unwrap();

        // With force, should replace the symlink
        create_symlink_with_force(&source, &target, false, true).unwrap();

        // Verify the symlink now points to the new source
        assert!(is_stau_symlink(&target, &source).unwrap());
        assert!(!is_stau_symlink(&target, &old_source).unwrap());
    }

    #[test]
    fn test_force_respects_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        // Create source and target files
        File::create(&source).unwrap();
        fs::write(&target, "existing content").unwrap();

        // With force and dry_run, should succeed but not modify anything
        create_symlink_with_force(&source, &target, true, true).unwrap();

        // Verify the file still exists and wasn't replaced
        assert!(!target.symlink_metadata().unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "existing content");
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
}
