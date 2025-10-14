use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StauError>;

#[derive(Error, Debug)]
pub enum StauError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Conflicting file exists: {0}")]
    ConflictingFile(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Setup script failed for package {package}: {message}")]
    SetupScriptFailed { package: String, message: String },

    #[error("Teardown script failed for package {package}: {message}")]
    TeardownScriptFailed { package: String, message: String },

    #[error("STAU_DIR not found: {0}")]
    StauDirNotFound(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("Broken symlink: {0}")]
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
