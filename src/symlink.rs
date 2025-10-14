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
    if dest.exists() {
        return Err(StauError::ConflictingFile(dest.to_path_buf()));
    }

    if dry_run {
        return Ok(());
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
}
