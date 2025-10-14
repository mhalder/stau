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

    #[test]
    fn test_config_creation() {
        // This test will only pass if STAU_DIR or ~/dotfiles exists
        if let Ok(cfg) = Config::new() {
            assert!(cfg.stau_dir.exists());
        }
    }
}
