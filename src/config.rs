use crate::error::{Result, StauError};
use std::env;
use std::path::PathBuf;

/// Configuration for stau, handles STAU_DIR and STAU_TARGET environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory where dotfiles are stored (default: ~/.config/dotfiles or ~/dotfiles)
    pub stau_dir: PathBuf,
    /// Default target directory for symlinks (default: $HOME)
    pub default_target: PathBuf,
}

impl Config {
    /// Creates a new Config by reading environment variables and resolving default paths.
    ///
    /// STAU_DIR resolution priority:
    /// 1. `$STAU_DIR` environment variable
    /// 2. `~/.config/dotfiles` (XDG-compliant)
    /// 3. `~/dotfiles` (legacy)
    pub fn new() -> Result<Self> {
        let stau_dir = Self::get_stau_dir()?;
        let default_target = Self::get_default_target()?;

        Ok(Config {
            stau_dir,
            default_target,
        })
    }

    /// Get STAU_DIR from environment or use default paths
    /// Priority: $STAU_DIR > ~/.config/dotfiles > ~/dotfiles
    fn get_stau_dir() -> Result<PathBuf> {
        if let Ok(dir) = env::var("STAU_DIR") {
            let path = PathBuf::from(dir);
            if path.exists() {
                Ok(path)
            } else {
                Err(StauError::StauDirNotFound(path))
            }
        } else {
            let home = Self::get_home_dir()?;

            // XDG-compliant path: ~/.config/dotfiles (preferred)
            let xdg_dotfiles = home.join(".config").join("dotfiles");
            if xdg_dotfiles.exists() {
                return Ok(xdg_dotfiles);
            }

            // Legacy path: ~/dotfiles
            let legacy_dotfiles = home.join("dotfiles");
            if legacy_dotfiles.exists() {
                return Ok(legacy_dotfiles);
            }

            // Neither exists, report the XDG path as the preferred location
            Err(StauError::StauDirNotFound(xdg_dotfiles))
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

    /// Get the user's home directory (cross-platform)
    fn get_home_dir() -> Result<PathBuf> {
        // First try the dirs crate for cross-platform support
        if let Some(home) = dirs::home_dir() {
            return Ok(home);
        }

        // Fallback to HOME environment variable (Unix)
        env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| StauError::Other("Could not determine home directory".to_string()))
    }

    /// Gets the target directory, using provided override or the default target.
    ///
    /// If an override is provided, it takes precedence over the default target
    /// (which is `$STAU_TARGET` or `$HOME`).
    pub fn get_target(&self, override_target: Option<PathBuf>) -> PathBuf {
        override_target.unwrap_or_else(|| self.default_target.clone())
    }

    /// Gets the full path to a package directory within STAU_DIR.
    pub fn get_package_dir(&self, package: &str) -> PathBuf {
        self.stau_dir.join(package)
    }

    /// Checks if a package exists (i.e., its directory exists in STAU_DIR).
    pub fn package_exists(&self, package: &str) -> bool {
        self.get_package_dir(package).exists()
    }

    /// Gets the setup script path for a package, if it exists.
    ///
    /// Returns `Some(path)` if `<package>/setup.sh` exists and is a file, `None` otherwise.
    pub fn get_setup_script(&self, package: &str) -> Option<PathBuf> {
        let script_path = self.get_package_dir(package).join("setup.sh");
        if script_path.exists() && script_path.is_file() {
            Some(script_path)
        } else {
            None
        }
    }

    /// Gets the teardown script path for a package, if it exists.
    ///
    /// Returns `Some(path)` if `<package>/teardown.sh` exists and is a file, `None` otherwise.
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

    #[test]
    fn test_xdg_path_preferred_over_legacy() {
        let temp_dir = TempDir::new().unwrap();

        // Create both XDG and legacy paths
        let xdg_dotfiles = temp_dir.path().join(".config").join("dotfiles");
        let legacy_dotfiles = temp_dir.path().join("dotfiles");
        fs::create_dir_all(&xdg_dotfiles).unwrap();
        fs::create_dir(&legacy_dotfiles).unwrap();

        // Mock HOME to point to temp_dir
        temp_env::with_vars(
            vec![
                ("HOME", Some(temp_dir.path().to_str().unwrap())),
                ("STAU_DIR", None::<&str>),
            ],
            || {
                let config = Config::new().unwrap();
                // Should prefer XDG path over legacy
                assert_eq!(config.stau_dir, xdg_dotfiles);
            },
        );
    }

    #[test]
    fn test_legacy_path_fallback() {
        let temp_dir = TempDir::new().unwrap();

        // Only create legacy path
        let legacy_dotfiles = temp_dir.path().join("dotfiles");
        fs::create_dir(&legacy_dotfiles).unwrap();

        // Mock HOME to point to temp_dir
        temp_env::with_vars(
            vec![
                ("HOME", Some(temp_dir.path().to_str().unwrap())),
                ("STAU_DIR", None::<&str>),
            ],
            || {
                let config = Config::new().unwrap();
                // Should fall back to legacy path
                assert_eq!(config.stau_dir, legacy_dotfiles);
            },
        );
    }
}
