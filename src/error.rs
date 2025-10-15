use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StauError>;

#[derive(Error, Debug)]
pub enum StauError {
    #[error(
        "Package not found: {0}\nHint: Check that the package exists in your STAU_DIR. Use 'stau list' to see available packages."
    )]
    PackageNotFound(String),

    #[error(
        "Conflicting file exists: {0}\nHint: A file already exists at this location. Either:\n  - Remove the existing file manually\n  - Use --force to overwrite it (caution: this will delete the existing file)\n  - Adopt the existing file with 'stau adopt <package> {0}'"
    )]
    ConflictingFile(PathBuf),

    #[error(
        "Permission denied: {0}\nHint: You may need elevated privileges. Try running with 'sudo' or check file permissions."
    )]
    PermissionDenied(String),

    #[error(
        "Setup script failed for package {package}: {message}\nHint: Check the setup script at <STAU_DIR>/{package}/setup.sh for errors. You can skip the setup script with --no-setup."
    )]
    SetupScriptFailed { package: String, message: String },

    #[error(
        "Teardown script failed for package {package}: {message}\nHint: Check the teardown script at <STAU_DIR>/{package}/teardown.sh for errors. You can skip the teardown script with --no-teardown."
    )]
    TeardownScriptFailed { package: String, message: String },

    #[error(
        "STAU_DIR not found: {0}\nHint: Create your dotfiles directory or set the STAU_DIR environment variable to point to your existing dotfiles."
    )]
    StauDirNotFound(PathBuf),

    #[error("Invalid path: {0}\nHint: The specified path is invalid or inaccessible.")]
    InvalidPath(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Other(String),
}

impl StauError {
    pub fn exit_code(&self) -> i32 {
        match self {
            StauError::PackageNotFound(_) => 1,
            StauError::ConflictingFile(_) => 2,
            StauError::PermissionDenied(_) => 3,
            StauError::SetupScriptFailed { .. } => 4,
            StauError::TeardownScriptFailed { .. } => 4,
            StauError::StauDirNotFound(_) => 1,
            StauError::InvalidPath(_) => 1,
            StauError::Io(_) => 3,
            StauError::Other(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_package_not_found_error() {
        let err = StauError::PackageNotFound("vim".to_string());
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().contains("vim"));
        assert!(err.to_string().contains("stau list"));
    }

    #[test]
    fn test_conflicting_file_error() {
        let path = PathBuf::from("/home/user/.vimrc");
        let err = StauError::ConflictingFile(path.clone());
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("/home/user/.vimrc"));
        assert!(err.to_string().contains("--force"));
        assert!(err.to_string().contains("stau adopt"));
    }

    #[test]
    fn test_permission_denied_error() {
        let err = StauError::PermissionDenied("Cannot write to /root".to_string());
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("Cannot write to /root"));
        assert!(err.to_string().contains("sudo"));
    }

    #[test]
    fn test_setup_script_failed_error() {
        let err = StauError::SetupScriptFailed {
            package: "vim".to_string(),
            message: "script exited with code 1".to_string(),
        };
        assert_eq!(err.exit_code(), 4);
        assert!(err.to_string().contains("vim"));
        assert!(err.to_string().contains("script exited with code 1"));
        assert!(err.to_string().contains("--no-setup"));
    }

    #[test]
    fn test_teardown_script_failed_error() {
        let err = StauError::TeardownScriptFailed {
            package: "zsh".to_string(),
            message: "script exited with code 2".to_string(),
        };
        assert_eq!(err.exit_code(), 4);
        assert!(err.to_string().contains("zsh"));
        assert!(err.to_string().contains("script exited with code 2"));
        assert!(err.to_string().contains("--no-teardown"));
    }

    #[test]
    fn test_stau_dir_not_found_error() {
        let path = PathBuf::from("/home/user/dotfiles");
        let err = StauError::StauDirNotFound(path.clone());
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().contains("/home/user/dotfiles"));
        assert!(err.to_string().contains("STAU_DIR"));
    }

    #[test]
    fn test_invalid_path_error() {
        let path = PathBuf::from("/invalid/path");
        let err = StauError::InvalidPath(path.clone());
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().contains("/invalid/path"));
    }

    #[test]
    fn test_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = StauError::Io(io_err);
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_other_error() {
        let err = StauError::Other("Something went wrong".to_string());
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().contains("Something went wrong"));
    }

    #[test]
    fn test_error_conversion_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let stau_err: StauError = io_err.into();
        assert_eq!(stau_err.exit_code(), 3);
    }
}
