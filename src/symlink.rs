use crate::error::{Result, StauError, map_io_error};
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
    /// Creates a new symlink mapping from source to target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        Self { source, target }
    }
}

/// Checks if a path is a symlink pointing to the expected target.
///
/// Returns `Ok(true)` if path is a symlink pointing to expected_target,
/// `Ok(false)` if not a symlink or points elsewhere.
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

/// Checks if a symlink is broken (points to non-existent file).
///
/// Returns `true` if path is a symlink whose target doesn't exist,
/// `false` otherwise (including when path doesn't exist or isn't a symlink).
pub fn is_broken_symlink(path: &Path) -> bool {
    if let Ok(metadata) = path.symlink_metadata()
        && metadata.is_symlink()
    {
        return !path.exists();
    }
    false
}

/// Creates a symlink at `target` pointing to `source`, creating parent directories as needed.
///
/// If the symlink already exists and points to the correct source, this is a no-op.
/// Returns an error if a conflicting file exists at the target location.
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
            map_io_error(e, format!("Cannot create directory: {}", parent.display()))
        })?;
    }

    // Create the symlink
    unix_fs::symlink(source, target)
        .map_err(|e| map_io_error(e, format!("Cannot create symlink: {}", target.display())))?;

    Ok(())
}

/// Removes a symlink if it points to the expected source.
///
/// Returns `Ok(true)` if the symlink was removed, `Ok(false)` if it wasn't a stau-managed symlink.
/// Only removes symlinks that point to the expected source to avoid removing other symlinks.
pub fn remove_symlink(path: &Path, expected_source: &Path, dry_run: bool) -> Result<bool> {
    if !is_stau_symlink(path, expected_source)? {
        return Ok(false); // Not our symlink, don't remove
    }

    if dry_run {
        return Ok(true);
    }

    fs::remove_file(path)
        .map_err(|e| map_io_error(e, format!("Cannot remove symlink: {}", path.display())))?;

    Ok(true)
}

/// Copies a file from source to destination, creating parent directories as needed.
///
/// Returns an error if the destination already exists to prevent data loss.
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
            map_io_error(e, format!("Cannot create directory: {}", parent.display()))
        })?;
    }

    fs::copy(source, dest)
        .map_err(|e| map_io_error(e, format!("Cannot copy file: {}", dest.display())))?;

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
}
