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
            println!("Restowing package: {}", package);
            if cli.dry_run {
                println!("(dry run - no changes made)");
            }
            if run_setup {
                println!("Will run setup script");
            }
            // TODO: Implement restow logic
            Ok(())
        }

        Commands::Adopt {
            package,
            files,
            target,
        } => {
            println!("Adopting {} files into package: {}", files.len(), package);
            if cli.dry_run {
                println!("(dry run - no changes made)");
            }
            // TODO: Implement adopt logic
            Ok(())
        }

        Commands::List { target } => {
            println!("Listing packages...");
            // TODO: Implement list logic
            Ok(())
        }

        Commands::Status { package, target } => {
            println!("Status for package: {}", package);
            // TODO: Implement status logic
            Ok(())
        }

        Commands::Clean { package, target } => {
            println!("Cleaning package: {}", package);
            if cli.dry_run {
                println!("(dry run - no changes made)");
            }
            // TODO: Implement clean logic
            Ok(())
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
