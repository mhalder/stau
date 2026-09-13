use crate::error::{Result, StauError, map_io_error};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::Duration;

/// Executes a setup or teardown script for a package.
///
/// The script is run with the following environment variables set:
/// - `STAU_DIR`: Path to the dotfiles directory
/// - `STAU_PACKAGE`: Name of the package being installed/uninstalled
/// - `STAU_TARGET`: Target directory where symlinks are created
///
/// Returns an error if the script fails (non-zero exit code) or cannot be executed.
pub fn execute_script(
    script_path: &Path,
    package_name: &str,
    stau_dir: &Path,
    target_dir: &Path,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    if dry_run {
        println!("Would execute: {}", script_path.display());
        return Ok(());
    }

    if verbose {
        println!("Executing: {}", script_path.display());
    }

    let status =
        run_script_command(script_path, package_name, stau_dir, target_dir).map_err(|e| {
            map_io_error(
                e,
                format!(
                    "Cannot execute script: {}. Make sure it's executable (chmod +x)",
                    script_path.display()
                ),
            )
        })?;

    // Check exit status
    if !status.success() {
        use std::os::unix::process::ExitStatusExt;

        let script_type = if script_path.ends_with("setup.sh") {
            "setup"
        } else {
            "teardown"
        };

        let message = match (status.code(), status.signal()) {
            (Some(code), _) => format!("{} script failed with exit code {}", script_type, code),
            (None, Some(signal)) => {
                format!("{} script was terminated by signal {}", script_type, signal)
            }
            (None, None) => format!("{} script failed", script_type),
        };

        if script_type == "setup" {
            return Err(StauError::SetupScriptFailed {
                package: package_name.to_string(),
                message,
            });
        } else {
            return Err(StauError::TeardownScriptFailed {
                package: package_name.to_string(),
                message,
            });
        }
    }

    Ok(())
}

/// Runs the script with inherited stdin, stdout and stderr, so scripts can prompt
/// for input and their output is shown as it is produced.
fn run_script_command(
    script_path: &Path,
    package_name: &str,
    stau_dir: &Path,
    target_dir: &Path,
) -> io::Result<ExitStatus> {
    const MAX_ATTEMPTS: usize = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        let status = Command::new(script_path)
            .current_dir(target_dir)
            .env("STAU_DIR", stau_dir)
            .env("STAU_PACKAGE", package_name)
            .env("STAU_TARGET", target_dir)
            .status();

        match status {
            Ok(status) => return Ok(status),
            Err(error)
                if error.kind() == io::ErrorKind::ExecutableFileBusy && attempt < MAX_ATTEMPTS =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("retry loop always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn create_script(path: &Path, content: &str) {
        let mut file = File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.sync_all().unwrap(); // Ensure data is written to disk
        drop(file); // Explicitly close file before changing permissions

        // Make executable
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();

        // Sync directory to ensure metadata changes are persisted
        if let Some(parent) = path.parent()
            && let Ok(dir) = fs::File::open(parent)
        {
            let _ = dir.sync_all();
        }
    }

    #[test]
    fn test_execute_successful_script() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        create_script(&script_path, "#!/bin/bash\necho 'Setup running'\nexit 0\n");

        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, false, false);

        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_failing_setup_script() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        create_script(&script_path, "#!/bin/bash\nexit 1\n");

        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, false, false);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StauError::SetupScriptFailed { .. }
        ));
    }

    #[test]
    fn test_execute_failing_teardown_script() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("teardown.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        create_script(&script_path, "#!/bin/bash\nexit 1\n");

        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, false, false);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StauError::TeardownScriptFailed { .. }
        ));
    }

    #[test]
    fn test_dry_run_skips_execution() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create a script that would fail
        create_script(&script_path, "#!/bin/bash\nexit 1\n");

        // In dry run, it should not execute and should succeed
        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, true, false);

        assert!(result.is_ok());
    }

    #[test]
    fn test_script_receives_environment_variables() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");
        let output_file = temp_dir.path().join("env_vars.txt");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Script that writes env vars to a file
        create_script(
            &script_path,
            &format!(
                "#!/bin/bash\necho \"$STAU_DIR\" > {}\necho \"$STAU_PACKAGE\" >> {}\necho \"$STAU_TARGET\" >> {}\n",
                output_file.display(),
                output_file.display(),
                output_file.display()
            ),
        );

        execute_script(
            &script_path,
            "test_package",
            &stau_dir,
            &target_dir,
            false,
            false,
        )
        .unwrap();

        let contents = fs::read_to_string(&output_file).unwrap();
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines[0], stau_dir.to_str().unwrap());
        assert_eq!(lines[1], "test_package");
        assert_eq!(lines[2], target_dir.to_str().unwrap());
    }

    #[test]
    fn test_non_executable_script() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Create script without execute permissions
        let mut file = File::create(&script_path).unwrap();
        file.write_all(b"#!/bin/bash\necho test\n").unwrap();
        drop(file);

        // Should fail with permission denied
        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, false, false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StauError::PermissionDenied(_)
        ));
    }

    #[test]
    fn test_script_stdout_stderr_handling() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // Script that writes to both stdout and stderr
        create_script(
            &script_path,
            "#!/bin/bash\necho 'stdout message'\necho 'stderr message' >&2\nexit 0\n",
        );

        let result = execute_script(&script_path, "test", &stau_dir, &target_dir, false, false);
        assert!(result.is_ok());
    }
}
