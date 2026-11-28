//! Filesystem helpers shared by the modules that write files.

use crate::error::{Result, StauError, map_io_error};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` atomically: a unique temporary file in the same
/// directory, then `fs::rename` over the destination.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| StauError::InvalidPath(path.to_path_buf()))?;

    let mut tmp_path;
    let mut counter = 0u64;
    loop {
        tmp_path = dir.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            counter
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(mut file) => {
                let result = file
                    .write_all(bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|e| {
                        map_io_error(e, format!("Cannot write file: {}", tmp_path.display()))
                    });
                if result.is_err() {
                    let _ = fs::remove_file(&tmp_path);
                    return result;
                }
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                counter += 1;
            }
            Err(e) => {
                return Err(map_io_error(
                    e,
                    format!("Cannot create temporary file: {}", tmp_path.display()),
                ));
            }
        }
    }

    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        map_io_error(e, format!("Cannot write file: {}", path.display()))
    })
}

/// The regular file an atomic write must replace when `path` is a symlink.
///
/// A renamed temporary file replaces whatever sits at its destination, so
/// writing to a symlinked path would swap the link for a regular file and
/// break a dotfile-managed target. Resolving first keeps the link and rewrites
/// what it points at. A dangling link is resolved against its own parent,
/// because `canonicalize` rejects a destination that does not exist yet.
pub(crate) fn resolve_symlink_target(path: &Path) -> PathBuf {
    let is_symlink = fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    if !is_symlink {
        return path.to_path_buf();
    }

    match fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(_) => match fs::read_link(path) {
            Ok(dest) if dest.is_absolute() => dest,
            Ok(dest) => path.parent().unwrap_or_else(|| Path::new(".")).join(dest),
            Err(_) => path.to_path_buf(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_atomic_writes_the_bytes() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file");

        write_atomic(&path, b"contents").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"contents");
    }

    #[test]
    fn test_write_atomic_reports_a_missing_directory() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing").join("file");

        let err = write_atomic(&path, b"contents").unwrap_err();

        assert!(
            err.to_string().contains("Cannot create temporary file"),
            "message: {err}"
        );
    }

    #[test]
    fn test_resolve_symlink_target_follows_a_link() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real");
        fs::write(&real, "x").unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(resolve_symlink_target(&link), real);
    }

    #[test]
    fn test_resolve_symlink_target_passes_a_regular_path_through() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file");
        fs::write(&path, "x").unwrap();

        assert_eq!(resolve_symlink_target(&path), path);
    }
}
