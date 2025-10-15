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
    if let Some(parent) = path.parent()
        && let Ok(dir) = fs::File::open(parent)
    {
        let _ = dir.sync_all();
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
    let package_dir = stau_dir.join("zsh");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "zsh", &[".zshrc"]);

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
        .args(["install", "zsh", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall with teardown script
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "zsh"])
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
    let package_dir = stau_dir.join("zsh");
    fs::create_dir(&package_dir).unwrap();

    create_test_package(&stau_dir, "zsh", &[".zshrc"]);

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
        .args(["install", "zsh", "--no-setup"])
        .output()
        .unwrap();

    // Uninstall with --no-teardown
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["uninstall", "zsh", "--no-teardown"])
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
    let zshrc = target_dir.join(".zshrc");
    fs::write(&bashrc, "echo 'bash'").unwrap();
    fs::write(&zshrc, "echo 'zsh'").unwrap();

    // Adopt multiple files
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args([
            "adopt",
            "shell",
            bashrc.to_str().unwrap(),
            zshrc.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Adopt failed: {:?}", output);
    assert!(bashrc.is_symlink(), ".bashrc should be a symlink");
    assert!(zshrc.is_symlink(), ".zshrc should be a symlink");
    assert!(stau_dir.join("shell/.bashrc").exists());
    assert!(stau_dir.join("shell/.zshrc").exists());
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
fn test_restow_with_run_setup() {
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

    // Install first
    let _ = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["install", "vim", "--no-setup"])
        .output()
        .unwrap();

    // Restow with run-setup
    let output = Command::new(stau_binary())
        .env("STAU_DIR", &stau_dir)
        .env("STAU_TARGET", &target_dir)
        .args(["restow", "vim", "--run-setup"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(marker_file.exists(), "Setup script should have run");
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
