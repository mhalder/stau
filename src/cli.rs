use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "stau")]
#[command(
    version,
    about = "A modern dotfile manager with GNU Stow-style symlink management"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Dry run - show what would be done without making changes
    #[arg(short = 'n', long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand)]
pub enum Commands {
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

        /// Scan the whole target directory tree, also finding links below directories removed from the package (slower)
        #[arg(long)]
        deep: bool,
    },
}

impl Commands {
    /// Returns the package name given on the command line, if any.
    pub fn package(&self) -> Option<&str> {
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
