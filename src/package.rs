use crate::error::{Result, StauError};
use crate::symlink::SymlinkMapping;
use std::fs;
use std::path::Path;

/// Walk a package directory and generate symlink mappings
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
    walk_directory(package_dir, package_dir, target_dir, &mut mappings)?;
    Ok(mappings)
}

/// Recursively walk a directory and build symlink mappings
fn walk_directory(
    base_dir: &Path,
    current_dir: &Path,
    target_dir: &Path,
    mappings: &mut Vec<SymlinkMapping>,
) -> Result<()> {
    let entries = fs::read_dir(current_dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            StauError::PermissionDenied(format!("Cannot read directory: {}", current_dir.display()))
        } else {
            StauError::Io(e)
        }
    })?;

    for entry in entries {
        let entry = entry.map_err(StauError::Io)?;
        let path = entry.path();
        let file_name = entry.file_name();

        // Skip setup.sh and teardown.sh scripts
        if file_name == "setup.sh" || file_name == "teardown.sh" {
            continue;
        }

        // Skip version control files/directories in root of package
        let file_name_str = file_name.to_string_lossy();
        if current_dir == base_dir
            && matches!(
                file_name_str.as_ref(),
                ".git" | ".gitignore" | ".gitattributes" | ".gitmodules"
            ) {
                continue;
            }

        let metadata = entry.metadata().map_err(StauError::Io)?;

        if metadata.is_dir() {
            // Recursively walk subdirectories
            walk_directory(base_dir, &path, target_dir, mappings)?;
        } else if metadata.is_file() {
            // Calculate relative path from package base
            let rel_path = path
                .strip_prefix(base_dir)
                .map_err(|_| StauError::InvalidPath(path.clone()))?;

            // Target path is target_dir + relative path
            let target_path = target_dir.join(rel_path);

            mappings.push(SymlinkMapping::new(path, target_path));
        }
        // Skip symlinks and other special files
    }

    Ok(())
}

/// List all packages in the stau directory
pub fn list_packages(stau_dir: &Path) -> Result<Vec<String>> {
    if !stau_dir.exists() {
        return Err(StauError::StauDirNotFound(stau_dir.to_path_buf()));
    }

    let entries = fs::read_dir(stau_dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            StauError::PermissionDenied(format!("Cannot read directory: {}", stau_dir.display()))
        } else {
            StauError::Io(e)
        }
    })?;

    let mut packages = Vec::new();
    for entry in entries {
        let entry = entry.map_err(StauError::Io)?;
        let path = entry.path();

        // Only include directories, skip hidden directories
        if path.is_dir()
            && let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if !name_str.starts_with('.') {
                    packages.push(name_str.to_string());
                }
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
        assert!(mappings
            .iter()
            .any(|m| m.source.ends_with(".bashrc") && m.target.ends_with(".bashrc")));
        assert!(mappings
            .iter()
            .any(|m| m.source.ends_with(".vimrc") && m.target.ends_with(".vimrc")));
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
        fs::create_dir(stau_dir.join("zsh")).unwrap();
        fs::create_dir(stau_dir.join("vim")).unwrap();
        fs::create_dir(stau_dir.join("git")).unwrap();
        fs::create_dir(stau_dir.join(".hidden")).unwrap();

        let packages = list_packages(stau_dir).unwrap();

        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"zsh".to_string()));
        assert!(packages.contains(&"vim".to_string()));
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
}
