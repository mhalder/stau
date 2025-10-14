use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StauError>;

#[derive(Error, Debug)]
pub enum StauError {
    #[error("Package not found: {0}\nHint: Check that the package exists in your STAU_DIR. Use 'stau list' to see available packages.")]
    PackageNotFound(String),

    #[error("Conflicting file exists: {0}\nHint: A file already exists at this location. Either:\n  - Remove the existing file manually\n  - Use --force to overwrite it (caution: this will delete the existing file)\n  - Adopt the existing file with 'stau adopt <package> {0}'")]
    ConflictingFile(PathBuf),

    #[error("Permission denied: {0}\nHint: You may need elevated privileges. Try running with 'sudo' or check file permissions.")]
    PermissionDenied(String),

    #[error("Setup script failed for package {package}: {message}\nHint: Check the setup script at <STAU_DIR>/{package}/setup.sh for errors. You can skip the setup script with --no-setup.")]
    SetupScriptFailed { package: String, message: String },

    #[error("Teardown script failed for package {package}: {message}\nHint: Check the teardown script at <STAU_DIR>/{package}/teardown.sh for errors. You can skip the teardown script with --no-teardown.")]
    TeardownScriptFailed { package: String, message: String },

    #[error("STAU_DIR not found: {0}\nHint: Create your dotfiles directory or set the STAU_DIR environment variable to point to your existing dotfiles.")]
    StauDirNotFound(PathBuf),

    #[error("Invalid path: {0}\nHint: The specified path is invalid or inaccessible.")]
    InvalidPath(PathBuf),

    #[error("Broken symlink: {0}\nHint: Use 'stau clean <package>' to remove broken symlinks.")]
    BrokenSymlink(PathBuf),

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
            StauError::BrokenSymlink(_) => 1,
            StauError::Io(_) => 3,
            StauError::Other(_) => 1,
        }
    }
}
