use crate::error::{Result, StauError, map_io_error};
use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

/// How a source file is materialized at its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingKind {
    /// A GNU Stow-style symlink from target to source (the default).
    Symlink,
    /// A `*.tmpl` source rendered into a regular file at the target.
    Render,
}

/// Represents a mapping from a package source to a target location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkMapping {
    /// The source file in the package directory
    pub source: PathBuf,
    /// The target location where the symlink should be created (or file written)
    pub target: PathBuf,
    /// How the source is materialized at the target.
    pub kind: MappingKind,
    /// The rendered contents, `Some(..)` iff `kind == Render` and rendering succeeded.
    pub rendered: Option<String>,
    /// The render failure, `Some(..)` iff `kind == Render` and rendering failed.
    pub render_error: Option<String>,
}

impl SymlinkMapping {
    /// Creates a new symlink mapping from source to target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        Self {
            source,
            target,
            kind: MappingKind::Symlink,
            rendered: None,
            render_error: None,
        }
    }

    /// Creates a render mapping from source to target with rendered content.
    pub fn render(source: PathBuf, target: PathBuf, content: String) -> Self {
        Self {
            source,
            target,
            kind: MappingKind::Render,
            rendered: Some(content),
            render_error: None,
        }
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

/// Checks if a path is a broken symlink pointing into `package_dir`.
///
/// Relative link targets are resolved against the symlink's parent directory.
/// Broken symlinks pointing elsewhere are not stau-managed and return `false`.
pub fn is_broken_package_symlink(path: &Path, package_dir: &Path) -> bool {
    if !is_broken_symlink(path) {
        return false;
    }

    let Ok(link_target) = fs::read_link(path) else {
        return false;
    };

    let resolved = match path.parent() {
        Some(parent) if link_target.is_relative() => normalize_lexically(&parent.join(link_target)),
        _ => link_target,
    };
    resolved.starts_with(package_dir)
}

/// Resolves `.` and `..` components without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// Checks every target for conflicts before any mapping is applied.
///
/// This is the conflict pre-check used by `install` so a conflicting file does
/// not leave the package partially installed. It reports the first conflict.
/// For `Render` mappings this is a dry-run of [`write_rendered`].
pub fn check_conflicts(mappings: &[SymlinkMapping]) -> Result<()> {
    for mapping in mappings {
        match mapping.kind {
            MappingKind::Symlink => create_symlink(&mapping.source, &mapping.target, true)?,
            MappingKind::Render => write_rendered(
                &mapping.source,
                &mapping.target,
                mapping
                    .rendered
                    .as_deref()
                    .expect("render mapping carries content"),
                true,
            )?,
        }
    }
    Ok(())
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

/// Compares two files byte-for-byte.
///
/// Used to recognize a target file that stau copied back from the package on
/// uninstall (the `restored` state).
pub fn same_contents(a: &Path, b: &Path) -> Result<bool> {
    let contents_a =
        fs::read(a).map_err(|e| map_io_error(e, format!("Cannot read file: {}", a.display())))?;
    let contents_b =
        fs::read(b).map_err(|e| map_io_error(e, format!("Cannot read file: {}", b.display())))?;
    Ok(contents_a == contents_b)
}

/// Compares a file's bytes to a string.
///
/// Used to recognize a render target whose contents match the current render
/// (the `installed` state for a `Render` mapping).
pub fn same_bytes(path: &Path, content: &str) -> Result<bool> {
    let existing = fs::read(path)
        .map_err(|e| map_io_error(e, format!("Cannot read file: {}", path.display())))?;
    Ok(existing == content.as_bytes())
}

/// Returns `Ok(true)` when `target` is a regular file whose bytes equal `content`.
pub fn rendered_is_current(target: &Path, content: &str) -> Result<bool> {
    match target.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => same_bytes(target, content),
        Ok(_) => Ok(false),
        Err(_) => Ok(false),
    }
}

/// Returns `Ok(true)` when `target` is a regular file whose bytes differ from
/// `content`, i.e. local modifications that uninstall would discard.
pub fn rendered_differs(target: &Path, content: &str) -> Result<bool> {
    match target.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => Ok(!same_bytes(target, content)?),
        Ok(_) => Ok(false),
        Err(_) => Ok(false),
    }
}

/// Writes a rendered file at `target`, preserving the source's permission bits.
///
/// If the target already exists as a regular file with bytes equal to `content`,
/// only its permission bits are converged to the source's (so a `chmod -x` is
/// repaired). If it exists as anything else (symlink, directory, special file)
/// or as a regular file with different bytes, this returns
/// [`StauError::ConflictingFile`]. Otherwise the bytes are written atomically
/// (a temporary file in the same directory, then `fs::rename`) and the target's
/// permission bits are copied from `source` so the executable bit is preserved.
pub fn write_rendered(source: &Path, target: &Path, content: &str, dry_run: bool) -> Result<()> {
    match target.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => {
            if !same_bytes(target, content)? {
                return Err(StauError::ConflictingFile(target.to_path_buf()));
            }
            if dry_run {
                return Ok(());
            }
            return set_permissions_from_source(source, target);
        }
        Ok(_) => return Err(StauError::ConflictingFile(target.to_path_buf())),
        Err(_) => {}
    }

    if dry_run {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            map_io_error(e, format!("Cannot create directory: {}", parent.display()))
        })?;
    }

    write_atomic(target, content.as_bytes())?;
    set_permissions_from_source(source, target)
}

/// Copies the permission bits of `source` onto `target`.
fn set_permissions_from_source(source: &Path, target: &Path) -> Result<()> {
    let permissions = fs::metadata(source)
        .map_err(|e| map_io_error(e, format!("Cannot read metadata: {}", source.display())))?
        .permissions();
    fs::set_permissions(target, permissions)
        .map_err(|e| map_io_error(e, format!("Cannot set permissions: {}", target.display())))
}

/// Removes a rendered file, and only a rendered file.
///
/// Returns `Ok(true)` if a regular file was removed, `Ok(false)` if the target
/// is absent, a symlink, or a directory (never delete something stau did not
/// create).
pub fn remove_rendered(target: &Path, dry_run: bool) -> Result<bool> {
    match target.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => {
            if dry_run {
                return Ok(true);
            }
            fs::remove_file(target).map_err(|e| {
                map_io_error(e, format!("Cannot remove file: {}", target.display()))
            })?;
            Ok(true)
        }
        Ok(_) => Ok(false),
        Err(_) => Ok(false),
    }
}

/// Writes `bytes` to `path` atomically: a unique temporary file in the same
/// directory, then `fs::rename` over the destination.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
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

/// Moves a file, creating parent directories of the destination as needed.
///
/// Falls back to copying and removing the source when source and destination are
/// on different filesystems, where a rename is not possible.
pub fn move_file(source: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            map_io_error(e, format!("Cannot create directory: {}", parent.display()))
        })?;
    }

    match fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            fs::copy(source, dest)
                .map_err(|e| map_io_error(e, format!("Cannot copy file: {}", dest.display())))?;
            fs::remove_file(source)
                .map_err(|e| map_io_error(e, format!("Cannot remove file: {}", source.display())))
        }
        Err(e) => Err(map_io_error(
            e,
            format!("Cannot move file: {}", source.display()),
        )),
    }
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

    #[test]
    fn test_is_broken_package_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("pkg");
        let target_dir = temp_dir.path().join("home");
        fs::create_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        let existing = package_dir.join("existing");
        File::create(&existing).unwrap();

        let into_package = target_dir.join("into_package");
        unix_fs::symlink(package_dir.join("removed"), &into_package).unwrap();

        let relative_into_package = target_dir.join("relative");
        unix_fs::symlink("../pkg/removed", &relative_into_package).unwrap();

        let elsewhere = target_dir.join("elsewhere");
        unix_fs::symlink("/nonexistent/file", &elsewhere).unwrap();

        let valid = target_dir.join("valid");
        unix_fs::symlink(&existing, &valid).unwrap();

        assert!(is_broken_package_symlink(&into_package, &package_dir));
        assert!(!is_broken_package_symlink(&elsewhere, &package_dir));
        assert!(!is_broken_package_symlink(&valid, &package_dir));
        assert!(!is_broken_package_symlink(&existing, &package_dir));
        assert!(is_broken_package_symlink(
            &relative_into_package,
            &package_dir
        ));
    }

    #[test]
    fn test_move_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("nested/dir/dest.txt");
        fs::write(&source, "content").unwrap();

        move_file(&source, &dest).unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
    }

    #[test]
    fn test_move_file_across_filesystems() {
        use std::os::unix::fs::MetadataExt;

        // /dev/shm is usually a separate tmpfs mount, where rename fails with EXDEV
        let Ok(other_fs) = TempDir::new_in("/dev/shm") else {
            return;
        };
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = other_fs.path().join("dest.txt");
        fs::write(&source, "content").unwrap();

        let same_device = fs::metadata(temp_dir.path()).unwrap().dev()
            == fs::metadata(other_fs.path()).unwrap().dev();
        if same_device {
            return;
        }

        move_file(&source, &dest).unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
    }

    #[test]
    fn test_write_rendered_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("nested/target");
        fs::write(&source, "source").unwrap();

        write_rendered(&source, &target, "rendered", false).unwrap();

        assert!(target.is_file());
        assert_eq!(fs::read_to_string(&target).unwrap(), "rendered");
        assert!(target.parent().unwrap().is_dir());
    }

    #[test]
    fn test_write_rendered_equal_is_noop() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "source").unwrap();
        fs::write(&target, "same").unwrap();

        write_rendered(&source, &target, "same", false).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "same");
    }

    #[test]
    fn test_write_rendered_repairs_mode_when_bytes_equal() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "content").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&target, "content").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();

        write_rendered(&source, &target, "content", false).unwrap();

        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "executable bits repaired");
        assert_eq!(fs::read_to_string(&target).unwrap(), "content");
    }

    #[test]
    fn test_write_rendered_conflict_on_different_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "source").unwrap();
        fs::write(&target, "existing").unwrap();

        let result = write_rendered(&source, &target, "rendered", false);
        assert!(matches!(result.unwrap_err(), StauError::ConflictingFile(_)));
        // The existing file is untouched.
        assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
    }

    #[test]
    fn test_write_rendered_directory_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "source").unwrap();
        fs::create_dir(&target).unwrap();

        let result = write_rendered(&source, &target, "rendered", false);
        assert!(matches!(result.unwrap_err(), StauError::ConflictingFile(_)));
        assert!(target.is_dir());
    }

    #[test]
    fn test_write_rendered_preserves_exec_bit() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("bin.tmpl");
        let target = temp_dir.path().join("bin");
        fs::write(&source, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();

        write_rendered(&source, &target, "#!/bin/sh\n", false).unwrap();

        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "executable bits preserved");
    }

    #[test]
    fn test_write_rendered_dry_run_writes_nothing() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "source").unwrap();

        write_rendered(&source, &target, "rendered", true).unwrap();
        assert!(!target.exists());
    }

    #[test]
    fn test_remove_rendered_regular_file() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        fs::write(&target, "content").unwrap();

        assert!(remove_rendered(&target, false).unwrap());
        assert!(!target.exists());
    }

    #[test]
    fn test_remove_rendered_leaves_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        unix_fs::symlink("/some/where", &target).unwrap();

        assert!(!remove_rendered(&target, false).unwrap());
        assert!(target.symlink_metadata().unwrap().is_symlink());
    }

    #[test]
    fn test_remove_rendered_missing_is_false() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("missing");

        assert!(!remove_rendered(&target, false).unwrap());
    }

    #[test]
    fn test_check_conflicts_renders_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("src.tmpl");
        let target = temp_dir.path().join("target");
        fs::write(&source, "source").unwrap();
        fs::write(&target, "different").unwrap();

        let mappings = vec![SymlinkMapping::render(
            source.clone(),
            target.clone(),
            "rendered".to_string(),
        )];
        let result = check_conflicts(&mappings);
        assert!(matches!(result.unwrap_err(), StauError::ConflictingFile(_)));
    }
}
