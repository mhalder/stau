use crate::error::{Result, StauError, map_io_error};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Options controlling how a setup or teardown script is executed.
#[derive(Debug, Clone, Default)]
pub struct ScriptOptions {
    /// Extra environment variables to set for the script process.
    pub env: Vec<(String, String)>,
    /// Connect the script's stdin to `/dev/null` instead of the terminal.
    pub non_interactive: bool,
    /// Kill the script after this long.
    pub timeout: Option<Duration>,
    /// Redirect the script's stdout to stderr so `--json` output on stdout
    /// stays machine-readable.
    pub json: bool,
}

/// Parses a human duration such as `30s`, `5m`, `500ms`, or a bare number of seconds.
pub fn parse_duration(input: &str) -> std::result::Result<Duration, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("duration must not be empty".to_string());
    }

    let split = input
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(input.len());
    let (number, unit) = input.split_at(split);

    let value: u64 = number.parse().map_err(|_| {
        format!(
            "invalid duration '{input}': expected a number of seconds or a value like 30s or 5m"
        )
    })?;

    let seconds = match unit {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => value,
        "ms" | "msec" | "msecs" | "millisecond" | "milliseconds" => {
            return Ok(Duration::from_millis(value));
        }
        "m" | "min" | "mins" | "minute" | "minutes" => value
            .checked_mul(60)
            .ok_or_else(|| format!("duration '{input}' is too large"))?,
        "h" | "hr" | "hrs" | "hour" | "hours" => value
            .checked_mul(3600)
            .ok_or_else(|| format!("duration '{input}' is too large"))?,
        _ => return Err(format!("unknown duration unit '{unit}' in '{input}'")),
    };

    Ok(Duration::from_secs(seconds))
}

/// Executes a setup or teardown script for a package.
///
/// The script is run with the following environment variables set:
/// - `STAU_DIR`: Path to the dotfiles directory
/// - `STAU_PACKAGE`: Name of the package being installed/uninstalled
/// - `STAU_TARGET`: Target directory where symlinks are created
///
/// Returns an error if the script fails (non-zero exit code), times out, or
/// cannot be executed.
pub fn execute_script(
    script_path: &Path,
    package_name: &str,
    stau_dir: &Path,
    target_dir: &Path,
    dry_run: bool,
    verbose: bool,
    options: &ScriptOptions,
) -> Result<()> {
    if dry_run {
        println!("Would execute: {}", script_path.display());
        return Ok(());
    }

    // Scripts run in the target directory; a missing directory would otherwise be
    // reported as a misleading "Cannot execute script" error
    if !target_dir.is_dir() {
        return Err(StauError::InvalidPath(target_dir.to_path_buf()));
    }

    if verbose {
        println!("Executing: {}", script_path.display());
    }

    let status = match run_script_command(script_path, package_name, stau_dir, target_dir, options)
    {
        Ok(status) => status,
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
            let script_type = script_type(script_path);
            return Err(script_failure(script_type, package_name, e.to_string()));
        }
        Err(e) => {
            return Err(map_io_error(
                e,
                format!(
                    "Cannot execute script: {}. Make sure it's executable (chmod +x)",
                    script_path.display()
                ),
            ));
        }
    };

    // Check exit status
    if !status.success() {
        use std::os::unix::process::ExitStatusExt;

        let script_type = script_type(script_path);
        let message = match (status.code(), status.signal()) {
            (Some(code), _) => format!("{} script failed with exit code {}", script_type, code),
            (None, Some(signal)) => {
                format!("{} script was terminated by signal {}", script_type, signal)
            }
            (None, None) => format!("{} script failed", script_type),
        };

        return Err(script_failure(script_type, package_name, message));
    }

    Ok(())
}

/// Returns the script kind name from the script path.
fn script_type(script_path: &Path) -> &'static str {
    if script_path.ends_with("setup.sh") {
        "setup"
    } else {
        "teardown"
    }
}

/// Builds the appropriate setup/teardown failure error for a script kind.
fn script_failure(script_type: &str, package_name: &str, message: String) -> StauError {
    if script_type == "setup" {
        StauError::SetupScriptFailed {
            package: package_name.to_string(),
            message,
        }
    } else {
        StauError::TeardownScriptFailed {
            package: package_name.to_string(),
            message,
        }
    }
}

/// Runs the script. stdin is inherited so scripts can prompt, unless
/// non-interactive mode connects it to `/dev/null`. stdout and stderr are
/// always inherited so output is shown as it is produced.
fn run_script_command(
    script_path: &Path,
    package_name: &str,
    stau_dir: &Path,
    target_dir: &Path,
    options: &ScriptOptions,
) -> io::Result<ExitStatus> {
    const MAX_ATTEMPTS: usize = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        let result = spawn_script(script_path, package_name, stau_dir, target_dir, options);
        match result {
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

/// Spawns the script once, applying the configured environment, stdin, and timeout.
fn spawn_script(
    script_path: &Path,
    package_name: &str,
    stau_dir: &Path,
    target_dir: &Path,
    options: &ScriptOptions,
) -> io::Result<ExitStatus> {
    let mut command = Command::new(script_path);
    command
        .current_dir(target_dir)
        .env("STAU_DIR", stau_dir)
        .env("STAU_PACKAGE", package_name)
        .env("STAU_TARGET", target_dir);

    for (key, value) in &options.env {
        command.env(key, value);
    }

    if options.non_interactive {
        command.stdin(Stdio::null());
    }

    if options.json {
        redirect_stdout_to_stderr(&mut command)?;
    }

    match options.timeout {
        Some(timeout) => run_with_timeout(command, timeout),
        None => command.status(),
    }
}

/// Duplicates stderr onto the command's stdout so the hook's own output lands
/// on stderr and leaves stdout free for one machine-readable object.
fn redirect_stdout_to_stderr(command: &mut Command) -> io::Result<()> {
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use std::os::unix::io::FromRawFd;

    let dup = unsafe { libc::dup(std::io::stderr().as_raw_fd()) };
    if dup < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: dup is a newly owned file descriptor handed to File.
    let file = unsafe { File::from_raw_fd(dup) };
    command.stdout(Stdio::from(file));
    Ok(())
}

/// Runs a spawned command in its own process group, killing the whole group if
/// it exceeds `timeout` (a hook's grandchildren must not outlive it).
fn run_with_timeout(mut command: Command, timeout: Duration) -> io::Result<ExitStatus> {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
    let mut child = command.spawn()?;
    let start = Instant::now();

    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None => {
                if start.elapsed() >= timeout {
                    terminate_process_group(child.id(), Duration::from_millis(500));
                    let _ = child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("script timed out after {timeout:?}"),
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Sends SIGTERM to the script's process group, waits `grace`, then escalates
/// to SIGKILL so even signal-ignoring descendants are reaped.
fn terminate_process_group(pgid: u32, grace: Duration) {
    let pgid = -(pgid as i32);
    // SAFETY: kill(-pgid, sig) signals the process group; ESRCH is harmless
    // when the group already exited during the grace period.
    unsafe {
        libc::kill(pgid, libc::SIGTERM);
    }
    thread::sleep(grace);
    unsafe {
        libc::kill(pgid, libc::SIGKILL);
    }
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
    fn test_parse_duration() {
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("10x").is_err());
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

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions::default(),
        );

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

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions::default(),
        );

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

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions::default(),
        );

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
        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            true,
            false,
            &ScriptOptions::default(),
        );

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
            &ScriptOptions::default(),
        )
        .unwrap();

        let contents = fs::read_to_string(&output_file).unwrap();
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines[0], stau_dir.to_str().unwrap());
        assert_eq!(lines[1], "test_package");
        assert_eq!(lines[2], target_dir.to_str().unwrap());
    }

    #[test]
    fn test_script_receives_extra_environment_variables() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");
        let output_file = temp_dir.path().join("custom_env.txt");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        create_script(
            &script_path,
            &format!(
                "#!/bin/bash\necho \"$MY_VAR\" > {}\n",
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
            &ScriptOptions {
                env: vec![("MY_VAR".to_string(), "hello".to_string())],
                ..ScriptOptions::default()
            },
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&output_file).unwrap().trim(), "hello");
    }

    #[test]
    fn test_script_timeout_kills_script() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        create_script(&script_path, "#!/bin/bash\nsleep 30\n");

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions {
                timeout: Some(Duration::from_millis(100)),
                ..ScriptOptions::default()
            },
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StauError::SetupScriptFailed { .. }
        ));
    }

    #[test]
    fn test_script_timeout_kills_grandchildren() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("setup.sh");
        let stau_dir = temp_dir.path().join("stau");
        let target_dir = temp_dir.path().join("target");
        let heartbeat = temp_dir.path().join("heartbeat");

        fs::create_dir(&stau_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();

        // A grandchild writes a heartbeat until the whole process group is
        // killed; the parent script itself just sleeps past the timeout.
        create_script(
            &script_path,
            &format!(
                "#!/bin/bash\n( while :; do echo beat >> {}; sleep 0.05; done ) &\nsleep 30\n",
                heartbeat.display()
            ),
        );

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions {
                timeout: Some(Duration::from_millis(300)),
                ..ScriptOptions::default()
            },
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StauError::SetupScriptFailed { .. }
        ));

        let heartbeats = || {
            fs::read_to_string(&heartbeat)
                .map(|s| s.lines().count())
                .unwrap_or(0)
        };
        let before = heartbeats();
        assert!(before > 0, "grandchild should have written heartbeats");
        thread::sleep(Duration::from_millis(400));
        let after = heartbeats();
        assert_eq!(before, after, "grandchild must be dead after the timeout");
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
        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions::default(),
        );
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

        let result = execute_script(
            &script_path,
            "test",
            &stau_dir,
            &target_dir,
            false,
            false,
            &ScriptOptions::default(),
        );
        assert!(result.is_ok());
    }
}
