use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get the path to the stau binary
fn stau_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove 'deps'
    path.push("stau");
    path
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
    let package_dir = stau_dir.join("zsh");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "zsh", &[".zshrc"]);

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
        .args(["install", "zsh"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Install with setup failed: {:?}",
        output
    );
    assert!(marker_file.exists(), "Setup script didn't run");
    assert!(target_dir.join(".zshrc").is_symlink());
}

#[test]
fn test_install_no_setup_flag() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    let package_dir = stau_dir.join("zsh");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "zsh", &[".zshrc"]);

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
        .args(["install", "zsh", "--no-setup"])
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
fn test_force_flag_overwrites_file() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    create_test_package(&stau_dir, "vim", &[".vimrc"]);

    // Create conflicting file
    fs::write(target_dir.join(".vimrc"), "existing content").unwrap();

    // Install without force - should fail
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "Should fail without --force");

    // Install with force - should succeed
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--force"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Should succeed with --force: stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target_dir.join(".vimrc").is_symlink());
}

#[test]
fn test_force_flag_overwrites_directory() {
    let temp_dir = TempDir::new().unwrap();
    let stau_dir = temp_dir.path().join("dotfiles");
    let target_dir = temp_dir.path().join("home");

    fs::create_dir(&stau_dir).unwrap();
    fs::create_dir(&target_dir).unwrap();

    // Create a package where the package directory itself will conflict
    let package_dir = stau_dir.join("config");
    fs::create_dir(&package_dir).unwrap();
    fs::write(package_dir.join(".config"), "config file").unwrap();

    // Create a conflicting directory at the exact target path
    let conflict_dir = target_dir.join(".config");
    fs::create_dir(&conflict_dir).unwrap();
    fs::write(conflict_dir.join("old_file.txt"), "old content").unwrap();

    // Install without force - should fail
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "config"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "Should fail without --force");

    // Install with force - should succeed and remove directory
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "config", "--force"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Should succeed with --force: stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target_dir.join(".config").is_symlink());
    assert!(!conflict_dir.join("old_file.txt").exists());
}

#[test]
fn test_uninstall_force_flag() {
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

    // Verify symlink was created
    assert!(target_dir.join(".vimrc").is_symlink());

    // Test that uninstall with --force flag is accepted and works
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "vim", "--force"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Uninstall with --force should succeed: stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The file should exist (copied back) and not be a symlink
    assert!(target_dir.join(".vimrc").exists());
    assert!(!target_dir.join(".vimrc").is_symlink());
}
