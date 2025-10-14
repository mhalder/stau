use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod config;
mod error;
mod package;
mod script;
mod symlink;

use config::Config;
use error::Result;

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

    /// Uninstall a package by removing symlinks and copying files back
    Uninstall {
        /// Package name to uninstall
        package: String,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running teardown script
        #[arg(long)]
        no_teardown: bool,

        /// Force uninstall even if conflicts exist
        #[arg(long)]
        force: bool,
    },

    /// Restow a package (uninstall and reinstall)
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
    List {
        /// Target directory to check status (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },

    /// Show detailed status for a specific package
    Status {
        /// Package name to show status for
        package: String,

        /// Target directory to check status (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,
    },

    /// Clean up broken symlinks for a package
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
        process::exit(1);
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
            force,
        } => uninstall_package(
            &config,
            &package,
            target,
            no_teardown,
            force,
            cli.dry_run,
            cli.verbose,
        ),

        Commands::Restow {
            package,
            target,
            run_setup,
        } => {
            // Uninstall first (without teardown by default)
            uninstall_package(
                &config,
                &package,
                target.clone(),
                true,
                false,
                cli.dry_run,
                cli.verbose,
            )?;

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
            "Successfully installed {} ({}  symlinks created)",
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

fn uninstall_package(
    config: &Config,
    package: &str,
    target: Option<PathBuf>,
    no_teardown: bool,
    _force: bool,
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

    // Run teardown script first if it exists and not skipped
    if !no_teardown && let Some(teardown_script) = config.get_teardown_script(package) {
        if verbose {
            println!("Found teardown script: {}", teardown_script.display());
        }

        // Note: PRD says teardown should continue even if it fails
        if let Err(e) = script::execute_script(
            &teardown_script,
            package,
            &config.stau_dir,
            &target_dir,
            dry_run,
            verbose,
        ) {
            eprintln!("Warning: Teardown script failed: {}", e);
            eprintln!("Continuing with uninstall...");
        } else if !dry_run {
            println!("Teardown script completed successfully");
        }
    }

    // Discover all files that would be in the package
    let mappings = package::discover_package_files(&package_dir, &target_dir)?;

    if verbose {
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
        let was_removed = symlink::remove_symlink(&mapping.target, &mapping.source, dry_run)?;

        if was_removed {
            if verbose || dry_run {
                println!("  Removing symlink: {}", mapping.target.display());
            }

            // Copy the source file to target location
            if verbose || dry_run {
                println!("  Copying file: {}", mapping.target.display());
            }

            symlink::copy_file(&mapping.source, &mapping.target, dry_run)?;
            removed_count += 1;
        } else if verbose {
            println!(
                "  Skipping {} (not a stau-managed symlink)",
                mapping.target.display()
            );
        }
    }

    if !dry_run {
        println!(
            "Successfully uninstalled {} ({} symlinks removed, files copied back)",
            package, removed_count
        );
    }

    Ok(())
}

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
                        if let Ok(is_our_link) =
                            symlink::is_stau_symlink(&mapping.target, &mapping.source)
                            && is_our_link
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
            Err(_) => {
                println!("  {:<20} [error reading package]", pkg);
            }
        }
    }

    Ok(())
}

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
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    error::StauError::PermissionDenied(format!(
                        "Cannot create package directory: {}",
                        package_dir.display()
                    ))
                } else {
                    error::StauError::Io(e)
                }
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
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        error::StauError::PermissionDenied(format!(
                            "Cannot remove symlink: {}",
                            mapping.target.display()
                        ))
                    } else {
                        error::StauError::Io(e)
                    }
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
