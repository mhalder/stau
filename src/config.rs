use crate::error::{Result, StauError};
use std::env;
use std::path::PathBuf;

/// Configuration for stau, handles STAU_DIR and STAU_TARGET environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where dotfiles are stored (default: ~/dotfiles)
    pub stau_dir: PathBuf,
    /// Default target directory for symlinks (default: $HOME)
    pub default_target: PathBuf,
}

impl Config {
    /// Create a new Config by reading environment variables
    pub fn new() -> Result<Self> {
        let stau_dir = Self::get_stau_dir()?;
        let default_target = Self::get_default_target()?;

        Ok(Config {
            stau_dir,
            default_target,
        })
    }

    /// Get STAU_DIR from environment or use default ~/dotfiles
    fn get_stau_dir() -> Result<PathBuf> {
        if let Ok(dir) = env::var("STAU_DIR") {
            let path = PathBuf::from(dir);
            if path.exists() {
                Ok(path)
            } else {
                Err(StauError::StauDirNotFound(path))
            }
        } else {
            // Default to ~/dotfiles
            let home = Self::get_home_dir()?;
            let dotfiles = home.join("dotfiles");
            if dotfiles.exists() {
                Ok(dotfiles)
            } else {
                Err(StauError::StauDirNotFound(dotfiles))
            }
        }
    }

    /// Get default target directory from STAU_TARGET or use $HOME
    fn get_default_target() -> Result<PathBuf> {
        if let Ok(target) = env::var("STAU_TARGET") {
            Ok(PathBuf::from(target))
        } else {
            Self::get_home_dir()
        }
    }

    /// Get the user's home directory
    fn get_home_dir() -> Result<PathBuf> {
        env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| StauError::Other("HOME environment variable not set".to_string()))
    }

    /// Get the target directory, using provided override or default
    pub fn get_target(&self, override_target: Option<PathBuf>) -> PathBuf {
        override_target.unwrap_or_else(|| self.default_target.clone())
    }

    /// Get the package directory path
    pub fn get_package_dir(&self, package: &str) -> PathBuf {
        self.stau_dir.join(package)
    }

    /// Check if a package exists
    pub fn package_exists(&self, package: &str) -> bool {
        self.get_package_dir(package).exists()
    }

    /// Get the setup script path for a package
    pub fn get_setup_script(&self, package: &str) -> Option<PathBuf> {
        let script_path = self.get_package_dir(package).join("setup.sh");
        if script_path.exists() && script_path.is_file() {
            Some(script_path)
        } else {
            None
        }
    }

    /// Get the teardown script path for a package
    pub fn get_teardown_script(&self, package: &str) -> Option<PathBuf> {
        let script_path = self.get_package_dir(package).join("teardown.sh");
        if script_path.exists() && script_path.is_file() {
            Some(script_path)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_with_stau_dir_env() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        // Set STAU_DIR environment variable
        temp_env::with_var("STAU_DIR", Some(stau_dir.to_str().unwrap()), || {
            let config = Config::new().unwrap();
            assert_eq!(config.stau_dir, stau_dir);
        });
    }

    #[test]
    fn test_config_stau_dir_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent");

        temp_env::with_var("STAU_DIR", Some(nonexistent.to_str().unwrap()), || {
            let result = Config::new();
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), StauError::StauDirNotFound(_)));
        });
    }

    #[test]
    fn test_config_with_stau_target_env() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&stau_dir).unwrap();

        temp_env::with_vars(
            vec![
                ("STAU_DIR", Some(stau_dir.to_str().unwrap())),
                ("STAU_TARGET", Some(target_dir.to_str().unwrap())),
            ],
            || {
                let config = Config::new().unwrap();
                assert_eq!(config.default_target, target_dir);
            },
        );
    }

    #[test]
    fn test_get_target_with_override() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        let default_target = temp_dir.path().join("default");
        let override_target = temp_dir.path().join("override");

        fs::create_dir(&stau_dir).unwrap();

        let config = Config {
            stau_dir,
            default_target: default_target.clone(),
        };

        // With override
        let target = config.get_target(Some(override_target.clone()));
        assert_eq!(target, override_target);

        // Without override
        let target = config.get_target(None);
        assert_eq!(target, default_target);
    }

    #[test]
    fn test_get_package_dir() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        let config = Config {
            stau_dir: stau_dir.clone(),
            default_target: temp_dir.path().to_path_buf(),
        };

        let package_dir = config.get_package_dir("vim");
        assert_eq!(package_dir, stau_dir.join("vim"));
    }

    #[test]
    fn test_package_exists() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        // Create a package
        let vim_dir = stau_dir.join("vim");
        fs::create_dir(&vim_dir).unwrap();

        let config = Config {
            stau_dir: stau_dir.clone(),
            default_target: temp_dir.path().to_path_buf(),
        };

        assert!(config.package_exists("vim"));
        assert!(!config.package_exists("nonexistent"));
    }

    #[test]
    fn test_get_setup_script() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        let vim_dir = stau_dir.join("vim");
        fs::create_dir(&vim_dir).unwrap();

        // Create setup script
        let setup_script = vim_dir.join("setup.sh");
        fs::write(&setup_script, "#!/bin/bash\necho test").unwrap();

        let config = Config {
            stau_dir: stau_dir.clone(),
            default_target: temp_dir.path().to_path_buf(),
        };

        // Package with setup script
        let script = config.get_setup_script("vim");
        assert!(script.is_some());
        assert_eq!(script.unwrap(), setup_script);

        // Package without setup script
        let script = config.get_setup_script("git");
        assert!(script.is_none());
    }

    #[test]
    fn test_get_teardown_script() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        let vim_dir = stau_dir.join("vim");
        fs::create_dir(&vim_dir).unwrap();

        // Create teardown script
        let teardown_script = vim_dir.join("teardown.sh");
        fs::write(&teardown_script, "#!/bin/bash\necho test").unwrap();

        let config = Config {
            stau_dir: stau_dir.clone(),
            default_target: temp_dir.path().to_path_buf(),
        };

        // Package with teardown script
        let script = config.get_teardown_script("vim");
        assert!(script.is_some());
        assert_eq!(script.unwrap(), teardown_script);

        // Package without teardown script
        let script = config.get_teardown_script("git");
        assert!(script.is_none());
    }

    #[test]
    fn test_setup_script_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let stau_dir = temp_dir.path().join("dotfiles");
        fs::create_dir(&stau_dir).unwrap();

        let vim_dir = stau_dir.join("vim");
        fs::create_dir(&vim_dir).unwrap();

        // Create setup.sh as a directory instead of a file
        let setup_dir = vim_dir.join("setup.sh");
        fs::create_dir(&setup_dir).unwrap();

        let config = Config {
            stau_dir: stau_dir.clone(),
            default_target: temp_dir.path().to_path_buf(),
        };

        // Should return None since setup.sh is not a file
        let script = config.get_setup_script("vim");
        assert!(script.is_none());
    }
}
