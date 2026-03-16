use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get the path to the stau binary
fn stau_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_stau"))
}

/// Helper to create a test package with files
fn create_test_package(stau_dir: &std::path::Path, package_name: &str, files: &[&str]) {
    let package_dir = stau_dir.join(package_name);
    fs::create_dir_all(&package_dir).unwrap();

    for file_path in files {
        let full_path = package_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(&full_path).unwrap();
        writeln!(file, "test content for {}", file_path).unwrap();
    }
}

/// Helper to create an executable script
fn create_script(path: &std::path::Path, content: &str) {
    let mut file = File::create(path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();

    // Sync directory
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

#[test]
fn test_install_and_uninstall_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create a test package
    create_test_package(&stau_dir, "vim", &[".vimrc", ".vim/colors/theme.vim"]);

    // Install the package
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Install failed: {:?}", output);
    assert!(target_dir.join(".vimrc").exists());
    assert!(target_dir.join(".vim/colors/theme.vim").exists());
    assert!(target_dir.join(".vimrc").is_symlink());

    // Uninstall the package
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Uninstall failed: {:?}", output);
    assert!(target_dir.join(".vimrc").exists());
    assert!(!target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_install_with_setup_script() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with setup script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("setup-ran");
    let setup_script = package_dir.join("setup.sh");
    create_script(
        &setup_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install with setup script
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Install with setup failed: {:?}",
        output
    );
    assert!(marker_file.exists(), "Setup script didn't run");
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_install_no_setup_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("setup-ran");
    let setup_script = package_dir.join("setup.sh");
    create_script(
        &setup_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install with --no-setup
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!marker_file.exists(), "Setup script ran when it shouldn't");
}

#[test]
fn test_list_command() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create multiple packages
    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    // Install only vim
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // List packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("vim"));
    assert!(stdout.contains("git"));
    assert!(stdout.contains("[installed]"));
    assert!(stdout.contains("[not installed]"));
}

#[test]
fn test_adopt_command() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create a file in target directory
    let config_file = target_dir.join(".bashrc");
    fs::write(&config_file, "echo 'hello'").unwrap();

    // Adopt the file
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "bash", config_file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Adopt failed: {:?}", output);
    assert!(config_file.is_symlink(), "File should be a symlink");
    assert!(
        stau_dir.join("bash/.bashrc").exists(),
        "File should be in package"
    );
}

#[test]
fn test_status_command() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Status before install
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["status", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not installed") || stdout.contains("Status for package"));
}

#[test]
fn test_dry_run_mode() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !target_dir.join(".vimrc").exists(),
        "Dry run should not create files"
    );
}

#[test]
fn test_conflict_detection() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Create conflicting file
    fs::write(target_dir.join(".vimrc"), "existing content").unwrap();

    // Try to install - should fail
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "Should fail due to conflict");
    assert_eq!(output.status.code().unwrap(), 2, "Should exit with code 2");
}

#[test]
fn test_restow_command() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Restow
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Restow failed: stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_package_not_found_error() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Try to install non-existent package
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "nonexistent"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code().unwrap(), 1, "Should exit with code 1");
}

#[test]
fn test_clean_command() {
    use std::os::unix::fs as unix_fs;

    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc", ".vim/plugin.vim"]);

    // Install
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Manually break the .vimrc symlink by removing it and creating a broken one
    let target_vimrc = target_dir.join(".vimrc");
    fs::remove_file(&target_vimrc).unwrap();

    // Create a symlink pointing to a non-existent file
    let broken_target = stau_dir.join("vim/.nonexistent");
    unix_fs::symlink(&broken_target, &target_vimrc).unwrap();

    // Verify we have a broken symlink
    assert!(target_vimrc.symlink_metadata().is_ok());
    assert!(!target_vimrc.exists()); // Broken symlink

    // Clean the broken symlinks
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["clean", "vim"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Clean should succeed: stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Broken symlink should be removed
    assert!(
        target_vimrc.symlink_metadata().is_err(),
        "Broken symlink should be completely removed"
    );

    // Good symlink should still exist
    assert!(target_dir.join(".vim/plugin.vim").is_symlink());
}

#[test]
fn test_clean_no_broken_symlinks() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Clean when there are no broken symlinks
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["clean", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No broken symlinks"));
}

#[test]
fn test_uninstall_with_teardown_script() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with teardown script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("teardown-ran");
    let teardown_script = package_dir.join("teardown.sh");
    create_script(
        &teardown_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall with teardown script
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Uninstall with teardown failed: {:?}",
        output
    );
    assert!(marker_file.exists(), "Teardown script didn't run");
}

#[test]
fn test_uninstall_no_teardown_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with teardown script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("teardown-ran");
    let teardown_script = package_dir.join("teardown.sh");
    create_script(
        &teardown_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall with --no-teardown
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim", "--no-teardown"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !marker_file.exists(),
        "Teardown script ran when it shouldn't"
    );
}

#[test]
fn test_teardown_script_failure_continues() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with failing teardown script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let teardown_script = package_dir.join("teardown.sh");
    create_script(&teardown_script, "#!/bin/bash\nexit 1\n");

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall - should succeed despite teardown failure
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Uninstall should succeed even if teardown fails"
    );

    // Verify uninstall still happened
    assert!(!target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_verbose_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Package directory:"));
    assert!(stdout.contains("Target directory:"));
    assert!(stdout.contains("STAU_DIR:"));
}

#[test]
fn test_adopt_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create files in target directory
    let bashrc = target_dir.join(".bashrc");
    let vimrc = target_dir.join(".vimrc");
    fs::write(&bashrc, "echo 'bash'").unwrap();
    fs::write(&vimrc, "echo 'vim'").unwrap();

    // Adopt multiple files
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args([
            "adopt",
            "shell",
            bashrc.to_str().unwrap(),
            vimrc.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Adopt failed: {:?}", output);
    assert!(bashrc.is_symlink(), ".bashrc should be a symlink");
    assert!(vimrc.is_symlink(), ".vimrc should be a symlink");
    assert!(stau_dir.join("shell/.bashrc").exists());
    assert!(stau_dir.join("shell/.vimrc").exists());
}

#[test]
fn test_partial_install_status() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc", ".vim/plugin.vim"]);

    // Install the package
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Remove one symlink to create partial install
    fs::remove_file(target_dir.join(".vimrc")).unwrap();

    // List should show partial status
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("vim"));
    assert!(stdout.contains("[partial]") || stdout.contains("1/2"));
}

#[test]
fn test_install_empty_package() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create empty package directory
    let empty_pkg = stau_dir.join("empty");
    fs::create_dir(&empty_pkg).unwrap();

    // Install empty package
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "empty"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No files to link"));
}

#[test]
fn test_list_with_empty_stau_dir() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // List with no packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No packages found"));
}

#[test]
fn test_list_with_broken_symlinks() {
    use std::os::unix::fs as unix_fs;

    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc", ".vim/plugin.vim"]);

    // Install
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Break one symlink
    let target_vimrc = target_dir.join(".vimrc");
    fs::remove_file(&target_vimrc).unwrap();
    unix_fs::symlink(stau_dir.join("vim/.nonexistent"), &target_vimrc).unwrap();

    // List should show broken status
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("broken") || stdout.contains("BROKEN"));
}

#[test]
fn test_adopt_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let nonexistent = target_dir.join(".nonexistent");

    // Try to adopt nonexistent file
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "test", nonexistent.to_str().unwrap()])
        .output()
        .unwrap();

    // Should succeed but warn about the file
    assert!(output.status.success());
}

#[test]
fn test_adopt_file_outside_target() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");
    let outside_file = temp_dir.path().join("outside.txt");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();
    fs::write(&outside_file, "content").unwrap();

    // Try to adopt file outside target directory
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "test", outside_file.to_str().unwrap()])
        .output()
        .unwrap();

    // Should succeed but skip the file
    assert!(output.status.success());
}

#[test]
fn test_adopt_with_existing_file_in_package() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with existing file
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    fs::write(package_dir.join(".vimrc"), "existing").unwrap();

    // Create file in target
    let vimrc = target_dir.join(".vimrc");
    fs::write(&vimrc, "new").unwrap();

    // Try to adopt - should fail due to conflict
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "vim", vimrc.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code().unwrap(), 2); // ConflictingFile error
}

#[test]
fn test_clean_with_dry_run() {
    use std::os::unix::fs as unix_fs;

    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Create broken symlink
    let target_vimrc = target_dir.join(".vimrc");
    fs::remove_file(&target_vimrc).unwrap();
    unix_fs::symlink(stau_dir.join("vim/.nonexistent"), &target_vimrc).unwrap();

    // Clean with dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["clean", "vim", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // Broken symlink should still exist
    assert!(target_vimrc.symlink_metadata().is_ok());
}

#[test]
fn test_restow_runs_setup_by_default() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with setup script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("setup-ran");
    let setup_script = package_dir.join("setup.sh");
    create_script(
        &setup_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first (without setup)
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Restow (setup runs by default now)
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        marker_file.exists(),
        "Setup script should have run by default"
    );
}

#[test]
fn test_restow_no_setup_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with setup script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("setup-ran");
    let setup_script = package_dir.join("setup.sh");
    create_script(
        &setup_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first (without setup)
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Restow with --no-setup
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--no-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !marker_file.exists(),
        "Setup script should NOT have run with --no-setup"
    );
}

#[test]
fn test_restow_runs_teardown_by_default() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with teardown script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("teardown-ran");
    let teardown_script = package_dir.join("teardown.sh");
    create_script(
        &teardown_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first (without setup)
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Restow (teardown runs by default now)
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--no-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        marker_file.exists(),
        "Teardown script should have run by default"
    );
}

#[test]
fn test_restow_no_teardown_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with teardown script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("teardown-ran");
    let teardown_script = package_dir.join("teardown.sh");
    create_script(
        &teardown_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first (without setup)
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Restow with --no-teardown
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--no-teardown", "--no-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !marker_file.exists(),
        "Teardown script should NOT have run with --no-teardown"
    );
}

#[test]
fn test_uninstall_empty_package() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create empty package
    let empty_pkg = stau_dir.join("empty");
    fs::create_dir(&empty_pkg).unwrap();

    // Uninstall empty package
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "empty"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No symlinks to remove"));
}

#[test]
fn test_status_with_conflict() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Create conflicting file (not a symlink)
    fs::write(target_dir.join(".vimrc"), "conflict").unwrap();

    // Status should show conflict
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["status", "vim"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[conflict]") || stdout.contains("not installed"));
}

#[test]
fn test_install_with_setup_script_failure() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with failing setup script
    let package_dir = stau_dir.join("vim");
    fs::create_dir(&package_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let setup_script = package_dir.join("setup.sh");
    create_script(&setup_script, "#!/bin/bash\nexit 1\n");

    // Install should fail
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code().unwrap(), 4); // SetupScriptFailed error
}

// Tests for --target CLI option
#[test]
fn test_install_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install using --target flag instead of env var
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["install", "vim", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Install with --target failed");
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_uninstall_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Uninstall using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["uninstall", "vim", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Uninstall with --target failed");
    assert!(!target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_restow_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Restow using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["restow", "vim", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Restow with --target failed");
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_adopt_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let config_file = target_dir.join(".bashrc");
    fs::write(&config_file, "echo 'hello'").unwrap();

    // Adopt using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args([
            "adopt",
            "bash",
            config_file.to_str().unwrap(),
            "--target",
            target_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Adopt with --target failed");
    assert!(config_file.is_symlink());
}

#[test]
fn test_list_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // List using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["list", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "List with --target failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("vim"));
    assert!(stdout.contains("[installed]"));
}

#[test]
fn test_status_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Status using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["status", "vim", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Status with --target failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Status for package"));
}

#[test]
fn test_clean_with_target_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Clean using --target flag
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .args(["clean", "vim", "--target", target_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "Clean with --target failed");
}

// Tests for --verbose with other commands
#[test]
fn test_uninstall_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Uninstall with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim", "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Package directory:") || stdout.contains("Removing symlink:"));
}

#[test]
fn test_restow_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Restow with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Package directory:") || stdout.contains("Target directory:"));
}

#[test]
fn test_adopt_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let config_file = target_dir.join(".bashrc");
    fs::write(&config_file, "echo 'hello'").unwrap();

    // Adopt with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "bash", config_file.to_str().unwrap(), "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verbose should show the file paths
    assert!(stdout.contains(".bashrc") || stdout.contains("bash"));
}

#[test]
fn test_clean_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Clean with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["clean", "vim", "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // With verbose, should output something even if no broken symlinks
    assert!(!output.stdout.is_empty() || !output.stderr.is_empty());
}

// Tests for --dry-run with other commands
#[test]
fn test_uninstall_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Uninstall with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim", "--dry-run"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Uninstall dry-run failed: stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Symlink should still exist (dry run doesn't actually uninstall)
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_restow_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    // Restow with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // Symlink should still exist
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_adopt_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let config_file = target_dir.join(".bashrc");
    fs::write(&config_file, "echo 'hello'").unwrap();

    // Adopt with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["adopt", "bash", config_file.to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // File should not be a symlink (dry run doesn't actually adopt)
    assert!(!config_file.is_symlink());
    // Package directory should not be created
    assert!(!stau_dir.join("bash").exists());
}

// =============================================================================
// Tests for install --all
// =============================================================================

#[test]
fn test_install_all_packages() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create multiple packages
    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);
    create_test_package(&stau_dir, "bash", &[".bashrc"]);

    // Install all packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Install --all failed: {:?}",
        output
    );

    // All packages should be installed
    assert!(target_dir.join(".vimrc").is_symlink());
    assert!(target_dir.join(".gitconfig").is_symlink());
    assert!(target_dir.join(".bashrc").is_symlink());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installing 3 packages"));
    assert!(stdout.contains("Successfully installed all 3 packages"));
}

#[test]
fn test_install_all_with_empty_stau_dir() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Install --all with no packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No packages found"));
}

#[test]
fn test_install_all_with_no_setup() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create packages with setup scripts
    let vim_dir = stau_dir.join("vim");
    fs::create_dir(&vim_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("setup-ran");
    let setup_script = vim_dir.join("setup.sh");
    create_script(
        &setup_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install --all with --no-setup
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all", "--no-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(target_dir.join(".vimrc").is_symlink());
    assert!(!marker_file.exists(), "Setup script should NOT have run");
}

#[test]
fn test_install_all_with_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    // Install --all with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());

    // No files should be created (dry run)
    assert!(!target_dir.join(".vimrc").exists());
    assert!(!target_dir.join(".gitconfig").exists());
}

#[test]
fn test_install_all_partial_failure() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create packages
    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    // Create a conflict for vim
    fs::write(target_dir.join(".vimrc"), "existing content").unwrap();

    // Install --all should continue despite one failure
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    // Command succeeds overall (returns success even with partial failures)
    assert!(output.status.success());

    // git should be installed despite vim failure
    assert!(target_dir.join(".gitconfig").is_symlink());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 failed"));
}

#[test]
fn test_install_all_and_package_mutual_exclusion() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Try to use both --all and a package name
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all", "vim"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot specify both --all and a package name"));
}

#[test]
fn test_install_requires_package_or_all() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Try to install without package or --all
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Must specify either a package name or --all"));
}

// =============================================================================
// Tests for uninstall --all
// =============================================================================

#[test]
fn test_uninstall_all_packages() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create and install multiple packages
    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);
    create_test_package(&stau_dir, "bash", &[".bashrc"]);

    // Install all first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    // Verify they're installed
    assert!(target_dir.join(".vimrc").is_symlink());
    assert!(target_dir.join(".gitconfig").is_symlink());
    assert!(target_dir.join(".bashrc").is_symlink());

    // Uninstall all packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Uninstall --all failed: {:?}",
        output
    );

    // All files should be regular files now (not symlinks)
    assert!(!target_dir.join(".vimrc").is_symlink());
    assert!(!target_dir.join(".gitconfig").is_symlink());
    assert!(!target_dir.join(".bashrc").is_symlink());

    // Files should still exist (copied back)
    assert!(target_dir.join(".vimrc").exists());
    assert!(target_dir.join(".gitconfig").exists());
    assert!(target_dir.join(".bashrc").exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Uninstalling 3 packages"));
    assert!(stdout.contains("Successfully uninstalled all 3 packages"));
}

#[test]
fn test_uninstall_all_with_empty_stau_dir() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Uninstall --all with no packages
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No packages found"));
}

#[test]
fn test_uninstall_all_with_no_teardown() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create package with teardown script
    let vim_dir = stau_dir.join("vim");
    fs::create_dir(&vim_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let marker_file = target_dir.join("teardown-ran");
    let teardown_script = vim_dir.join("teardown.sh");
    create_script(
        &teardown_script,
        &format!("#!/bin/bash\ntouch {}\n", marker_file.display()),
    );

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall --all with --no-teardown
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all", "--no-teardown"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!target_dir.join(".vimrc").is_symlink());
    assert!(!marker_file.exists(), "Teardown script should NOT have run");
}

#[test]
fn test_uninstall_all_with_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    // Install all first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    // Uninstall --all with --dry-run
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());

    // Symlinks should still exist (dry run doesn't actually uninstall)
    assert!(target_dir.join(".vimrc").is_symlink());
    assert!(target_dir.join(".gitconfig").is_symlink());
}

#[test]
fn test_uninstall_all_partial_failure() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create packages
    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);
    create_test_package(&stau_dir, "nonexistent_in_source", &[".config"]);

    // Install only vim and git
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "git"])
        .output()
        .unwrap();

    // Delete the source file for nonexistent_in_source to create a scenario where
    // uninstall will work but may encounter issues
    fs::remove_dir_all(stau_dir.join("nonexistent_in_source")).unwrap();
    fs::create_dir(stau_dir.join("nonexistent_in_source")).unwrap();

    // Uninstall --all should continue despite one having no symlinks
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all"])
        .output()
        .unwrap();

    // Command succeeds overall
    assert!(output.status.success());

    // vim and git should be uninstalled
    assert!(!target_dir.join(".vimrc").is_symlink());
    assert!(!target_dir.join(".gitconfig").is_symlink());
}

#[test]
fn test_uninstall_all_and_package_mutual_exclusion() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Try to use both --all and a package name
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all", "vim"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot specify both --all and a package name"));
}

#[test]
fn test_uninstall_requires_package_or_all() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Try to uninstall without package or --all
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Must specify either a package name or --all"));
}

#[test]
fn test_uninstall_all_runs_teardown_scripts() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create packages with teardown scripts
    let vim_dir = stau_dir.join("vim");
    fs::create_dir(&vim_dir).unwrap();
    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    let git_dir = stau_dir.join("git");
    fs::create_dir(&git_dir).unwrap();
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    let vim_marker = target_dir.join("vim-teardown-ran");
    let git_marker = target_dir.join("git-teardown-ran");

    create_script(
        &vim_dir.join("teardown.sh"),
        &format!("#!/bin/bash\ntouch {}\n", vim_marker.display()),
    );
    create_script(
        &git_dir.join("teardown.sh"),
        &format!("#!/bin/bash\ntouch {}\n", git_marker.display()),
    );

    // Install all first (without setup)
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall --all (teardown should run by default)
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(vim_marker.exists(), "Vim teardown script should have run");
    assert!(git_marker.exists(), "Git teardown script should have run");
}

#[test]
fn test_uninstall_all_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);
    create_test_package(&stau_dir, "git", &[".gitconfig"]);

    // Install all first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "--all"])
        .output()
        .unwrap();

    // Uninstall --all with --verbose
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "--all", "--verbose"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verbose output should show package-by-package progress
    assert!(stdout.contains("--- Uninstalling package:"));
    assert!(stdout.contains("Package directory:") || stdout.contains("STAU_DIR:"));
}
