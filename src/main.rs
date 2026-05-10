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
        return Ok(());
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
/// for remaining packages.
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
/// for remaining packages.
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

fn remove_package_symlinks(
    mappings: &[symlink::SymlinkMapping],
    opts: &UninstallOptions,
) -> Result<usize> {
    let mut removed_count = 0;

    for mapping in mappings {
        if remove_package_symlink(mapping, opts)? {
            removed_count += 1;
        }
    }

    Ok(removed_count)
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

    if opts.copy_files_back {
        copy_source_to_target(mapping, opts)?;
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
    let package_dir = config.get_package_dir(pkg);

    match package::discover_package_files(&package_dir, target_dir) {
        Ok(mappings) => print_package_status_from_mappings(pkg, &mappings),
        Err(e) => println!("  {:<20} [error: {}]", pkg, e),
    }
}

fn print_package_status_from_mappings(pkg: &str, mappings: &[symlink::SymlinkMapping]) {
    let status = package_status(mappings);

    if status.installed_count == 0 {
        println!("  {:<20} [not installed]", pkg);
    } else if status.broken_count > 0 {
        println!(
            "  {:<20} [installed]  {} symlinks  ({} broken)",
            pkg, status.installed_count, status.broken_count
        );
    } else if status.installed_count == status.total_count {
        println!(
            "  {:<20} [installed]  {} symlink{}",
            pkg,
            status.installed_count,
            plural_suffix(status.installed_count)
        );
    } else {
        println!(
            "  {:<20} [partial]    {}/{} symlinks",
            pkg, status.installed_count, status.total_count
        );
    }
}

fn package_status(mappings: &[symlink::SymlinkMapping]) -> PackageStatus {
    let installed_count = mappings
        .iter()
        .filter(|mapping| {
            matches!(
                symlink::is_stau_symlink(&mapping.target, &mapping.source),
                Ok(true)
            )
        })
        .count();
    let broken_count = mappings
        .iter()
        .filter(|mapping| symlink::is_broken_symlink(&mapping.target))
        .count();

    PackageStatus {
        total_count: mappings.len(),
        installed_count,
        broken_count,
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Adopts existing files into a package by moving them to the package directory and creating symlinks.
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

    ensure_package_dir(&package_dir, dry_run, verbose)?;
    println!(
        "Adopting {} file(s) into package '{}':",
        files.len(),
        package
    );

    for file_path in files {
        adopt_file(file_path, &target_dir, &package_dir, dry_run, verbose)?;
    }

    if !dry_run {
        println!(
            "Successfully adopted {} file(s) into '{}'",
            files.len(),
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

fn adopt_file(
    file_path: &Path,
    target_dir: &Path,
    package_dir: &Path,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let Some(dest) = adopt_destination(file_path, target_dir, package_dir)? else {
        return Ok(());
    };

    if verbose || dry_run {
        println!("  {} -> {}", file_path.display(), dest.display());
    }

    if !dry_run {
        move_file_into_package(file_path, &dest)?;
    }

    Ok(())
}

fn adopt_destination(
    file_path: &Path,
    target_dir: &Path,
    package_dir: &Path,
) -> Result<Option<PathBuf>> {
    if !file_path.exists() {
        eprintln!("Warning: File does not exist: {}", file_path.display());
        return Ok(None);
    }

    let Ok(rel_path) = file_path.strip_prefix(target_dir) else {
        eprintln!(
            "Warning: File {} is not in target directory {}",
            file_path.display(),
            target_dir.display()
        );
        return Ok(None);
    };

    let dest = package_dir.join(rel_path);
    if dest.exists() {
        return Err(error::StauError::ConflictingFile(dest));
    }

    Ok(Some(dest))
}

fn move_file_into_package(file_path: &Path, dest: &Path) -> Result<()> {
    use std::fs;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(error::StauError::Io)?;
    }

    fs::rename(file_path, dest).map_err(error::StauError::Io)?;
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

    if mappings.is_empty() {
        println!("\nNo files in package.");
        return Ok(());
    }

    println!("\nFiles ({} total):", mappings.len());

    let mut installed = 0;
    let mut not_installed = 0;
    let mut broken = 0;

    for mapping in &mappings {
        let is_our_link = symlink::is_stau_symlink(&mapping.target, &mapping.source)?;
        let is_broken = symlink::is_broken_symlink(&mapping.target);

        let status = if is_broken {
            broken += 1;
            "[BROKEN]"
        } else if is_our_link {
            installed += 1;
            "[installed]"
        } else if mapping.target.exists() {
            not_installed += 1;
            "[conflict]"
        } else {
            not_installed += 1;
            "[not installed]"
        };

        println!("  {:<20} {}", status, mapping.target.display());
    }

    println!();
    println!(
        "Summary: {} installed, {} not installed, {} broken",
        installed, not_installed, broken
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

    let mappings = package::discover_package_files(&package_dir, &target_dir)?;
    let mut cleaned = 0;

    for mapping in &mappings {
        if symlink::is_broken_symlink(&mapping.target) {
            if verbose || dry_run {
                println!("  Removing broken symlink: {}", mapping.target.display());
            }

            if !dry_run {
                fs::remove_file(&mapping.target).map_err(|e| {
                    map_io_error(
                        e,
                        format!("Cannot remove symlink: {}", mapping.target.display()),
                    )
                })?;
            }

            cleaned += 1;
        }
    }

    if cleaned == 0 {
        println!("No broken symlinks found for package '{}'", package);
    } else if !dry_run {
        println!(
            "Cleaned {} broken symlink(s) for package '{}'",
            cleaned, package
        );
    }

    Ok(())
}
