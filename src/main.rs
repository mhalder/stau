use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod config;
mod error;

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
        } => {
            println!("Installing package: {}", package);
            if cli.dry_run {
                println!("(dry run - no changes made)");
            }
            if no_setup {
                println!("Skipping setup script");
            }
            // TODO: Implement install logic
            Ok(())
        }

        Commands::Uninstall {
            package,
            target,
            no_teardown,
            force,
        } => {
            println!("Uninstalling package: {}", package);
            if cli.dry_run {
                println!("(dry run - no changes made)");
            }
            if no_teardown {
                println!("Skipping teardown script");
            }
            if force {
                println!("Force uninstall enabled");
            }
            // TODO: Implement uninstall logic
            Ok(())
        }

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
