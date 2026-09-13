use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process;

mod config;
mod error;
mod package;
mod script;
mod symlink;

use config::Config;
use error::{Result, map_io_error};

#[derive(Parser)]
#[command(name = "stau")]
#[command(
    version,
    about = "A modern dotfile manager with GNU Stow-style symlink management"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Dry run - show what would be done without making changes
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package by creating symlinks
    #[command(visible_alias = "i", visible_alias = "add")]
    Install {
        /// Package name to install (required unless --all is specified)
        package: Option<String>,

        /// Install all packages
        #[arg(short, long)]
        all: bool,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running setup script
        #[arg(long)]
        no_setup: bool,
    },

    /// Uninstall a package (removes symlinks and restores original files from dotfiles repo)
    #[command(visible_alias = "u", visible_alias = "rm")]
    Uninstall {
        /// Package name to uninstall (required unless --all is specified)
        package: Option<String>,

        /// Uninstall all packages
        #[arg(short, long)]
        all: bool,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running teardown script
        #[arg(long)]
        no_teardown: bool,
    },

    /// Restow a package (uninstall and reinstall)
    #[command(visible_alias = "r")]
    Restow {
        /// Package name to restow
        package: String,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running setup script
        #[arg(long)]
        no_setup: bool,

        /// Skip running teardown script
        #[arg(long)]
        no_teardown: bool,
    },

    /// Adopt existing files into a package
    #[command(visible_alias = "a")]
    Adopt {
        /// Package name to adopt files into
        package: String,

        /// File paths to adopt
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },

    /// List all packages and their installation status
    #[command(visible_alias = "l", visible_alias = "ls")]
    List {
        /// Target directory to check status (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },

    /// Show detailed status for a specific package
    #[command(visible_alias = "s")]
    Status {
        /// Package name to show status for
        package: String,

        /// Target directory to check status (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },

    /// Clean up broken symlinks for a package
    #[command(visible_alias = "c")]
    Clean {
        /// Package name to clean
        package: String,

        /// Target directory to clean (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },
}

impl Commands {
    /// Returns the package name given on the command line, if any.
    fn package(&self) -> Option<&str> {
        match self {
            Commands::Install { package, .. } | Commands::Uninstall { package, .. } => {
                package.as_deref()
            }
            Commands::Restow { package, .. }
            | Commands::Adopt { package, .. }
            | Commands::Status { package, .. }
            | Commands::Clean { package, .. } => Some(package),
            Commands::List { .. } => None,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);

        // Use appropriate exit code based on error type
        let exit_code = e.exit_code();

        process::exit(exit_code);
    }
}

fn run(cli: Cli) -> Result<()> {
    let config = Config::new()?;

    if cli.verbose {
        println!("STAU_DIR: {}", config.stau_dir.display());
    }

    dispatch_command(&config, cli.command, cli.dry_run, cli.verbose)
}

fn dispatch_command(
    config: &Config,
    command: Commands,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    if let Some(package) = command.package() {
        Config::validate_package_name(package)?;
    }

    match command {
        Commands::Install {
            package,
            all,
            target,
            no_setup,
        } => run_install(config, package, all, target, no_setup, dry_run, verbose),
        Commands::Uninstall {
            package,
            all,
            target,
            no_teardown,
        } => run_uninstall(config, package, all, target, no_teardown, dry_run, verbose),
        Commands::Restow {
            package,
            target,
            no_setup,
            no_teardown,
        } => restow_package(
            config,
            &package,
            target,
            no_setup,
            no_teardown,
            dry_run,
            verbose,
        ),
        Commands::Adopt {
            package,
            files,
            target,
        } => adopt_files(config, &package, &files, target, dry_run, verbose),
        Commands::List { target } => list_packages(config, target),
        Commands::Status { package, target } => show_status(config, &package, target),
        Commands::Clean { package, target } => {
            clean_broken_symlinks(config, &package, target, dry_run, verbose)
        }
    }
}

fn require_single_or_all(package: Option<String>, all: bool) -> Result<Option<String>> {
    match (all, package) {
        (true, Some(_)) => Err(error::StauError::InvalidArguments(
            "Cannot specify both --all and a package name".to_string(),
        )),
        (false, None) => Err(error::StauError::InvalidArguments(
            "Must specify either a package name or --all".to_string(),
        )),
        (_, package) => Ok(package),
    }
}

fn run_install(
    config: &Config,
    package: Option<String>,
    all: bool,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let package = require_single_or_all(package, all)?;

    if all {
        install_all_packages(config, target, no_setup, dry_run, verbose)
    } else {
        install_package(
            config,
            &package.expect("validated package"),
            target,
            no_setup,
            dry_run,
            verbose,
        )
    }
}

fn run_uninstall(
    config: &Config,
    package: Option<String>,
    all: bool,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let package = require_single_or_all(package, all)?;

    if all {
        uninstall_all_packages(config, target, no_teardown, dry_run, verbose)
    } else {
        uninstall_package(
            config,
            &package.expect("validated package"),
            target,
            no_teardown,
            dry_run,
            verbose,
        )
    }
}

fn restow_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_setup: bool,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: false,
        dry_run,
        verbose,
    };
    uninstall_package_internal(config, package, target.clone(), opts)?;
    install_package(config, package, target, no_setup, dry_run, verbose)
}

/// Installs a package by creating symlinks from the package directory to the target directory.
///
/// If a setup script exists and `no_setup` is false, it will be executed after creating symlinks.
fn install_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if verbose {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    // Check if package exists
    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    // Discover all files in the package
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;

    if verbose {
        println!("Found {} files to link", mappings.len());
    }

    if mappings.is_empty() {
        println!("No files to link in package '{}'", package);
    } else {
        // Check every target for conflicts before creating any symlink, so a
        // conflict does not leave the package partially installed
        for mapping in &mappings {
            symlink::create_symlink(&mapping.source, &mapping.target, true)?;
        }

        // Create symlinks for all files
        for mapping in &mappings {
            if verbose || dry_run {
                println!(
                    "  {} -> {}",
                    mapping.target.display(),
                    mapping.source.display()
                );
            }

            symlink::create_symlink(&mapping.source, &mapping.target, dry_run)?;
        }

        if !dry_run {
            println!(
                "Successfully installed {} ({} symlinks created)",
                package,
                mappings.len()
            );
        }
    }

    // Run setup script if it exists and not skipped
    if !no_setup && let Some(setup_script) = config.get_setup_script(package) {
        if verbose {
            println!("Found setup script: {}", setup_script.display());
        }

        script::execute_script(
            &setup_script,
            package,
            &config.stau_dir,
            &target_dir,
            dry_run,
            verbose,
        )?;

        if !dry_run {
            println!("Setup script completed successfully");
        }
    }

    Ok(())
}

/// Installs all packages found in STAU_DIR.
///
/// For each package, symlinks are created and setup scripts are run (unless skipped).
/// If any package fails to install, the error is reported but installation continues
/// for remaining packages, and an error is returned once all packages were processed.
fn install_all_packages(
    config: &Config,
    target: Option<PathBuf>,
    no_setup: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    run_for_all_packages(
        config,
        target,
        verbose,
        BatchLabels::new("Installing", "install", "installed", "Installed"),
        |pkg, target| install_package(config, pkg, target, no_setup, dry_run, verbose),
    )
}

/// Uninstalls all packages found in STAU_DIR.
///
/// For each package, teardown scripts are run (unless skipped) and symlinks are removed.
/// If any package fails to uninstall, the error is reported but uninstallation continues
/// for remaining packages, and an error is returned once all packages were processed.
fn uninstall_all_packages(
    config: &Config,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    run_for_all_packages(
        config,
        target,
        verbose,
        BatchLabels::new("Uninstalling", "uninstall", "uninstalled", "Uninstalled"),
        |pkg, target| uninstall_package(config, pkg, target, no_teardown, dry_run, verbose),
    )
}

struct BatchLabels {
    progress: &'static str,
    action: &'static str,
    success: &'static str,
    partial: &'static str,
}

impl BatchLabels {
    fn new(
        progress: &'static str,
        action: &'static str,
        success: &'static str,
        partial: &'static str,
    ) -> Self {
        Self {
            progress,
            action,
            success,
            partial,
        }
    }
}

fn run_for_all_packages<F>(
    config: &Config,
    target: Option<PathBuf>,
    verbose: bool,
    labels: BatchLabels,
    mut apply: F,
) -> Result<()>
where
    F: FnMut(&str, Option<PathBuf>) -> Result<()>,
{
    let packages = package::list_packages(&config.stau_dir)?;

    if packages.is_empty() {
        println!("No packages found in {}", config.stau_dir.display());
        return Ok(());
    }

    println!("{} {} packages...\n", labels.progress, packages.len());
    let (success_count, error_count) =
        apply_to_packages(&packages, target, verbose, &labels, &mut apply);
    print_batch_summary(success_count, error_count, &labels);

    if error_count > 0 {
        return Err(error::StauError::Other(format!(
            "Failed to {} {} of {} packages",
            labels.action,
            error_count,
            packages.len()
        )));
    }

    Ok(())
}

fn apply_to_packages<F>(
    packages: &[String],
    target: Option<PathBuf>,
    verbose: bool,
    labels: &BatchLabels,
    apply: &mut F,
) -> (usize, usize)
where
    F: FnMut(&str, Option<PathBuf>) -> Result<()>,
{
    let mut success_count = 0;
    let mut error_count = 0;

    for pkg in packages {
        if verbose {
            println!("--- {} package: {} ---", labels.progress, pkg);
        }

        match apply(pkg, target.clone()) {
            Ok(()) => success_count += 1,
            Err(e) => {
                eprintln!("Failed to {} '{}': {}", labels.action, pkg, e);
                error_count += 1;
            }
        }

        if verbose {
            println!();
        }
    }

    (success_count, error_count)
}

fn print_batch_summary(success_count: usize, error_count: usize, labels: &BatchLabels) {
    println!();
    if error_count == 0 {
        println!(
            "Successfully {} all {} packages",
            labels.success, success_count
        );
    } else {
        println!(
            "{} {} packages, {} failed",
            labels.partial, success_count, error_count
        );
    }
}

/// Options for uninstall operations.
struct UninstallOptions {
    /// Skip running the teardown script
    no_teardown: bool,
    /// Copy files back from package to target after removing symlinks
    copy_files_back: bool,
    /// Dry run mode - don't make actual changes
    dry_run: bool,
    /// Verbose output
    verbose: bool,
}

/// Uninstalls a package by removing symlinks and optionally copying files back.
///
/// If a teardown script exists and `no_teardown` is false, it will be executed first.
/// Teardown script failures are logged but don't stop the uninstall process.
fn uninstall_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_teardown: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let opts = UninstallOptions {
        no_teardown,
        copy_files_back: true,
        dry_run,
        verbose,
    };
    uninstall_package_internal(config, package, target, opts)
}

/// Internal implementation of package uninstallation with configurable options.
fn uninstall_package_internal(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    opts: UninstallOptions,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if opts.verbose {
        println!("Package directory: {}", package_dir.display());
        println!("Target directory: {}", target_dir.display());
    }

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    run_teardown_script(config, package, &target_dir, &opts);

    let mappings = package::discover_package_files(&package_dir, &target_dir)?;
    if opts.verbose {
        println!("Found {} symlinks to remove", mappings.len());
    }

    if mappings.is_empty() {
        println!("No symlinks to remove for package '{}'", package);
        return Ok(());
    }

    let removed_count = remove_package_symlinks(&mappings, &opts)?;
    print_uninstall_summary(package, removed_count, &opts);
    Ok(())
}

fn run_teardown_script(config: &Config, package: &str, target_dir: &Path, opts: &UninstallOptions) {
    if opts.no_teardown {
        return;
    }

    let Some(teardown_script) = config.get_teardown_script(package) else {
        return;
    };

    if opts.verbose {
        println!("Found teardown script: {}", teardown_script.display());
    }

    match script::execute_script(
        &teardown_script,
        package,
        &config.stau_dir,
        target_dir,
        opts.dry_run,
        opts.verbose,
    ) {
        Err(e) => {
            eprintln!("Warning: Teardown script failed: {}", e);
            eprintln!("Continuing with uninstall...");
        }
        Ok(()) if !opts.dry_run => println!("Teardown script completed successfully"),
        Ok(()) => {}
    }
}

/// Removes every stau symlink of a package, continuing past failures so one bad file
/// does not leave the rest of the package installed.
///
/// Every failure is reported; the last one is returned so its exit code is used.
fn remove_package_symlinks(
    mappings: &[symlink::SymlinkMapping],
    opts: &UninstallOptions,
) -> Result<usize> {
    let mut removed_count = 0;
    let mut errors = Vec::new();

    for mapping in mappings {
        match remove_package_symlink(mapping, opts) {
            Ok(true) => removed_count += 1,
            Ok(false) => {}
            Err(e) => errors.push(e),
        }
    }

    let Some(last_error) = errors.pop() else {
        return Ok(removed_count);
    };

    for e in &errors {
        eprintln!("Error: {}", e);
    }
    Err(last_error)
}

fn remove_package_symlink(
    mapping: &symlink::SymlinkMapping,
    opts: &UninstallOptions,
) -> Result<bool> {
    let was_removed = symlink::remove_symlink(&mapping.target, &mapping.source, opts.dry_run)?;

    if !was_removed {
        if opts.verbose {
            println!(
                "  Skipping {} (not a stau-managed symlink)",
                mapping.target.display()
            );
        }
        return Ok(false);
    }

    if opts.verbose || opts.dry_run {
        println!("  Removing symlink: {}", mapping.target.display());
    }

    if opts.copy_files_back
        && let Err(e) = copy_source_to_target(mapping, opts)
    {
        // The symlink is already gone; put it back so a failed copy does not
        // leave the target file missing
        if !opts.dry_run
            && let Err(restore_error) =
                symlink::create_symlink(&mapping.source, &mapping.target, false)
        {
            eprintln!(
                "Warning: Could not restore symlink {}: {}",
                mapping.target.display(),
                restore_error
            );
        }
        return Err(e);
    }

    Ok(true)
}

fn copy_source_to_target(mapping: &symlink::SymlinkMapping, opts: &UninstallOptions) -> Result<()> {
    if opts.verbose || opts.dry_run {
        println!("  Copying file: {}", mapping.target.display());
    }

    if !opts.dry_run && mapping.target.exists() {
        return Err(error::StauError::ConflictingFile(mapping.target.clone()));
    }

    if opts.dry_run && opts.verbose {
        println!("    (conflict check skipped in dry-run)");
    }

    symlink::copy_file(&mapping.source, &mapping.target, opts.dry_run)
}

fn print_uninstall_summary(package: &str, removed_count: usize, opts: &UninstallOptions) {
    if opts.dry_run {
        return;
    }

    if opts.copy_files_back {
        println!(
            "Successfully uninstalled {} ({} symlinks removed, files copied back)",
            package, removed_count
        );
    } else {
        println!(
            "Successfully removed {} symlinks for {}",
            removed_count, package
        );
    }
}

/// Lists all packages in STAU_DIR and their installation status.
fn list_packages(config: &Config, target: Option<PathBuf>) -> Result<()> {
    let target_dir = config.get_target(target);
    let packages = package::list_packages(&config.stau_dir)?;

    if packages.is_empty() {
        println!("No packages found in {}", config.stau_dir.display());
        return Ok(());
    }

    println!("Packages in {}:\n", config.stau_dir.display());
    for pkg in packages {
        print_package_status(config, &target_dir, &pkg);
    }

    Ok(())
}

struct PackageStatus {
    total_count: usize,
    installed_count: usize,
    broken_count: usize,
}

fn print_package_status(config: &Config, target_dir: &Path, pkg: &str) {
    match package_status(&config.get_package_dir(pkg), target_dir) {
        Ok(status) => print_package_status_line(pkg, &status),
        Err(e) => println!("  {:<20} [error: {}]", pkg, e),
    }
}

fn print_package_status_line(pkg: &str, status: &PackageStatus) {
    let broken = if status.broken_count > 0 {
        format!("  ({} broken)", status.broken_count)
    } else {
        String::new()
    };

    if status.installed_count == 0 {
        println!("  {:<20} [not installed]{}", pkg, broken);
    } else if status.installed_count == status.total_count {
        println!(
            "  {:<20} [installed]  {} symlink{}{}",
            pkg,
            status.installed_count,
            plural_suffix(status.installed_count),
            broken
        );
    } else {
        println!(
            "  {:<20} [partial]    {}/{} symlinks{}",
            pkg, status.installed_count, status.total_count, broken
        );
    }
}

fn package_status(package_dir: &Path, target_dir: &Path) -> Result<PackageStatus> {
    let mappings = package::discover_package_files(package_dir, target_dir)?;
    let installed_count = mappings
        .iter()
        .filter(|mapping| {
            matches!(
                symlink::is_stau_symlink(&mapping.target, &mapping.source),
                Ok(true)
            )
        })
        .count();

    Ok(PackageStatus {
        total_count: mappings.len(),
        installed_count,
        broken_count: package::discover_broken_symlinks(package_dir, target_dir)?.len(),
    })
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Adopts existing files into a package by moving them to the package directory and creating symlinks.
///
/// Every file is checked before any is moved, so a conflict does not leave the
/// package partially adopted.
fn adopt_files(
    config: &Config,
    package: &str,
    files: &[PathBuf],
    target: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    println!(
        "Adopting {} file(s) into package '{}':",
        files.len(),
        package
    );

    let mut adoptions: Vec<(PathBuf, PathBuf)> = Vec::new();
    for file_path in files {
        if let Some(adoption) = adopt_destination(file_path, &target_dir, &package_dir)?
            && !adoptions.iter().any(|(_, dest)| *dest == adoption.1)
        {
            adoptions.push(adoption);
        }
    }

    if adoptions.is_empty() {
        println!("No files adopted into '{}'", package);
        return Ok(());
    }

    ensure_package_dir(&package_dir, dry_run, verbose)?;

    for (file_path, dest) in &adoptions {
        if verbose || dry_run {
            println!("  {} -> {}", file_path.display(), dest.display());
        }

        if !dry_run {
            move_file_into_package(file_path, dest)?;
        }
    }

    if !dry_run {
        println!(
            "Successfully adopted {} file(s) into '{}'",
            adoptions.len(),
            package
        );
    }

    Ok(())
}

fn ensure_package_dir(package_dir: &Path, dry_run: bool, verbose: bool) -> Result<()> {
    use std::fs;

    if package_dir.exists() {
        return Ok(());
    }

    if verbose || dry_run {
        println!("Creating package directory: {}", package_dir.display());
    }

    if !dry_run {
        fs::create_dir_all(package_dir).map_err(|e| {
            map_io_error(
                e,
                format!("Cannot create package directory: {}", package_dir.display()),
            )
        })?;
    }

    Ok(())
}

/// Resolves where a file will be adopted to, returning `(absolute file path, destination)`.
///
/// Relative paths are resolved against the current directory. Missing files, files outside
/// the target directory, directories, and symlinks are skipped with a warning.
fn adopt_destination(
    file_path: &Path,
    target_dir: &Path,
    package_dir: &Path,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let file_path = std::path::absolute(file_path).map_err(error::StauError::Io)?;
    let target_dir = std::path::absolute(target_dir).map_err(error::StauError::Io)?;

    let Ok(metadata) = file_path.symlink_metadata() else {
        eprintln!("Warning: File does not exist: {}", file_path.display());
        return Ok(None);
    };

    if !metadata.is_file() {
        let kind = if metadata.is_dir() {
            "a directory"
        } else if metadata.is_symlink() {
            "a symlink"
        } else {
            "a special file"
        };
        eprintln!(
            "Warning: Skipping {} ({}); only regular files can be adopted",
            file_path.display(),
            kind
        );
        return Ok(None);
    }

    let Ok(rel_path) = file_path.strip_prefix(&target_dir) else {
        eprintln!(
            "Warning: File {} is not in target directory {}",
            file_path.display(),
            target_dir.display()
        );
        return Ok(None);
    };

    let dest = package_dir.join(rel_path);
    if dest.symlink_metadata().is_ok() {
        return Err(error::StauError::ConflictingFile(dest));
    }

    Ok(Some((file_path, dest)))
}

fn move_file_into_package(file_path: &Path, dest: &Path) -> Result<()> {
    symlink::move_file(file_path, dest)?;
    symlink::create_symlink(dest, file_path, false)
}

/// Shows detailed status for a specific package including symlink states and script availability.
fn show_status(config: &Config, package: &str, target: Option<PathBuf>) -> Result<()> {
    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    println!("Status for package '{}':\n", package);
    println!("  Package directory: {}", package_dir.display());
    println!("  Target directory:  {}", target_dir.display());

    // Check for setup/teardown scripts
    if let Some(setup) = config.get_setup_script(package) {
        println!("  Setup script:      {} (exists)", setup.display());
    } else {
        println!("  Setup script:      (none)");
    }

    if let Some(teardown) = config.get_teardown_script(package) {
        println!("  Teardown script:   {} (exists)", teardown.display());
    } else {
        println!("  Teardown script:   (none)");
    }

    // Get all mappings
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;
    let broken = package::discover_broken_symlinks(&package_dir, &target_dir)?;

    if mappings.is_empty() && broken.is_empty() {
        println!("\nNo files in package.");
        return Ok(());
    }

    println!("\nFiles ({} total):", mappings.len());

    let mut installed = 0;
    let mut not_installed = 0;

    for mapping in &mappings {
        let status = if broken.contains(&mapping.target) {
            "[BROKEN]"
        } else if symlink::is_stau_symlink(&mapping.target, &mapping.source)? {
            installed += 1;
            "[installed]"
        } else if mapping.target.symlink_metadata().is_ok() {
            // Includes dangling symlinks that do not point into the package
            not_installed += 1;
            "[conflict]"
        } else {
            not_installed += 1;
            "[not installed]"
        };

        println!("  {:<20} {}", status, mapping.target.display());
    }

    // Broken symlinks to files that were removed from the package
    for path in broken
        .iter()
        .filter(|path| !mappings.iter().any(|mapping| mapping.target == **path))
    {
        println!("  {:<20} {}", "[BROKEN]", path.display());
    }

    println!();
    println!(
        "Summary: {} installed, {} not installed, {} broken",
        installed,
        not_installed,
        broken.len()
    );

    Ok(())
}

/// Removes broken symlinks for a package.
fn clean_broken_symlinks(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    use std::fs;

    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    let broken = package::discover_broken_symlinks(&package_dir, &target_dir)?;

    for path in &broken {
        if verbose || dry_run {
            println!("  Removing broken symlink: {}", path.display());
        }

        if !dry_run {
            fs::remove_file(path).map_err(|e| {
                map_io_error(e, format!("Cannot remove symlink: {}", path.display()))
            })?;
        }
    }

    if broken.is_empty() {
        println!("No broken symlinks found for package '{}'", package);
    } else if !dry_run {
        println!(
            "Cleaned {} broken symlink(s) for package '{}'",
            broken.len(),
            package
        );
    }

    Ok(())
}
