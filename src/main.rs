use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
        /// Package name to install
        package: String,

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
        /// Package name to uninstall
        package: String,

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

        /// Run setup script during restow
        #[arg(long)]
        run_setup: bool,
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

    match cli.command {
        Commands::Install {
            package,
            target,
            no_setup,
        } => install_package(
            &config,
            &package,
            target,
            no_setup,
            cli.dry_run,
            cli.verbose,
        ),

        Commands::Uninstall {
            package,
            target,
            no_teardown,
        } => uninstall_package(
            &config,
            &package,
            target,
            no_teardown,
            cli.dry_run,
            cli.verbose,
        ),

        Commands::Restow {
            package,
            target,
            run_setup,
        } => {
            // Uninstall first (without teardown, without copying files back)
            let opts = UninstallOptions {
                no_teardown: true,
                copy_files_back: false, // Don't copy for restow!
                dry_run: cli.dry_run,
                verbose: cli.verbose,
            };
            uninstall_package_internal(&config, &package, target.clone(), opts)?;

            // Then install (with setup if requested)
            install_package(
                &config,
                &package,
                target,
                !run_setup,
                cli.dry_run,
                cli.verbose,
            )
        }

        Commands::Adopt {
            package,
            files,
            target,
        } => adopt_files(&config, &package, &files, target, cli.dry_run, cli.verbose),

        Commands::List { target } => list_packages(&config, target),

        Commands::Status { package, target } => show_status(&config, &package, target),

        Commands::Clean { package, target } => {
            clean_broken_symlinks(&config, &package, target, cli.dry_run, cli.verbose)
        }
    }
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

    // Check if package exists
    if !config.package_exists(package) {
        return Err(error::StauError::PackageNotFound(package.to_string()));
    }

    // Run teardown script first if it exists and not skipped
    if !opts.no_teardown
        && let Some(teardown_script) = config.get_teardown_script(package)
    {
        if opts.verbose {
            println!("Found teardown script: {}", teardown_script.display());
        }

        // Note: PRD says teardown should continue even if it fails
        if let Err(e) = script::execute_script(
            &teardown_script,
            package,
            &config.stau_dir,
            &target_dir,
            opts.dry_run,
            opts.verbose,
        ) {
            eprintln!("Warning: Teardown script failed: {}", e);
            eprintln!("Continuing with uninstall...");
        } else if !opts.dry_run {
            println!("Teardown script completed successfully");
        }
    }

    // Discover all files that would be in the package
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;

    if opts.verbose {
        println!("Found {} symlinks to remove", mappings.len());
    }

    if mappings.is_empty() {
        println!("No symlinks to remove for package '{}'", package);
        return Ok(());
    }

    let mut removed_count = 0;

    // Remove symlinks and copy files back
    for mapping in &mappings {
        // Remove the symlink if it points to our source
        let was_removed = symlink::remove_symlink(&mapping.target, &mapping.source, opts.dry_run)?;

        if was_removed {
            if opts.verbose || opts.dry_run {
                println!("  Removing symlink: {}", mapping.target.display());
            }

            // Copy the source file to target location (unless we're doing a restow)
            if opts.copy_files_back {
                if opts.verbose || opts.dry_run {
                    println!("  Copying file: {}", mapping.target.display());
                }

                // In dry-run mode, skip the conflict check since the symlink
                // wasn't actually removed yet
                if !opts.dry_run && mapping.target.exists() {
                    return Err(error::StauError::ConflictingFile(mapping.target.clone()));
                } else if opts.dry_run && opts.verbose {
                    println!("    (conflict check skipped in dry-run)");
                }

                symlink::copy_file(&mapping.source, &mapping.target, opts.dry_run)?;
            }
            removed_count += 1;
        } else if opts.verbose {
            println!(
                "  Skipping {} (not a stau-managed symlink)",
                mapping.target.display()
            );
        }
    }

    if !opts.dry_run {
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

    Ok(())
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
        let package_dir = config.get_package_dir(&pkg);

        // Check if package is installed by checking if any symlinks exist
        match package::discover_package_files(&package_dir, &target_dir) {
            Ok(mappings) => {
                if mappings.is_empty() {
                    println!("  {:<20} [not installed]", pkg);
                } else {
                    // Count how many are actually installed
                    let mut installed_count = 0;
                    let mut broken_count = 0;

                    for mapping in &mappings {
                        if let Ok(true) = symlink::is_stau_symlink(&mapping.target, &mapping.source)
                        {
                            installed_count += 1;
                        }

                        if symlink::is_broken_symlink(&mapping.target) {
                            broken_count += 1;
                        }
                    }

                    if installed_count == 0 {
                        println!("  {:<20} [not installed]", pkg);
                    } else if broken_count > 0 {
                        println!(
                            "  {:<20} [installed]  {} symlinks  ({} broken)",
                            pkg, installed_count, broken_count
                        );
                    } else if installed_count == mappings.len() {
                        println!(
                            "  {:<20} [installed]  {} symlink{}",
                            pkg,
                            installed_count,
                            if installed_count == 1 { "" } else { "s" }
                        );
                    } else {
                        println!(
                            "  {:<20} [partial]    {}/{} symlinks",
                            pkg,
                            installed_count,
                            mappings.len()
                        );
                    }
                }
            }
            Err(e) => {
                println!("  {:<20} [error: {}]", pkg, e);
            }
        }
    }

    Ok(())
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
    use std::fs;

    let target_dir = config.get_target(target);
    let package_dir = config.get_package_dir(package);

    // Create package directory if it doesn't exist
    if !package_dir.exists() {
        if verbose || dry_run {
            println!("Creating package directory: {}", package_dir.display());
        }
        if !dry_run {
            fs::create_dir_all(&package_dir).map_err(|e| {
                map_io_error(
                    e,
                    format!("Cannot create package directory: {}", package_dir.display()),
                )
            })?;
        }
    }

    println!(
        "Adopting {} file(s) into package '{}':",
        files.len(),
        package
    );

    for file_path in files {
        // Make sure the file exists
        if !file_path.exists() {
            eprintln!("Warning: File does not exist: {}", file_path.display());
            continue;
        }

        // Calculate relative path from target directory
        let rel_path = match file_path.strip_prefix(&target_dir) {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "Warning: File {} is not in target directory {}",
                    file_path.display(),
                    target_dir.display()
                );
                continue;
            }
        };

        // Destination in package directory
        let dest = package_dir.join(rel_path);

        // Check if destination already exists
        if dest.exists() {
            return Err(error::StauError::ConflictingFile(dest));
        }

        if verbose || dry_run {
            println!("  {} -> {}", file_path.display(), dest.display());
        }

        if !dry_run {
            // Create parent directories if needed
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(error::StauError::Io)?;
            }

            // Move the file
            fs::rename(file_path, &dest).map_err(error::StauError::Io)?;

            // Create symlink at original location
            symlink::create_symlink(&dest, file_path, false)?;
        }
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
