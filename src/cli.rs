use clap::{Parser, Subcommand, ValueEnum};
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

/// Options controlling how setup and teardown scripts run.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct ScriptArgs {
    /// Set an environment variable for hook scripts (repeatable)
    #[arg(long, value_name = "KEY=VALUE")]
    pub set: Vec<String>,

    /// Run hook scripts with stdin connected to /dev/null instead of the terminal
    #[arg(long)]
    pub non_interactive: bool,

    /// Kill a hook script that runs longer than this (e.g. 30s, 5m)
    #[arg(long, value_name = "DURATION")]
    pub script_timeout: Option<String>,
}

/// The state a `verify` run expects every file to be in.
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Expect {
    Installed,
    Uninstalled,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install packages by creating symlinks
    #[command(visible_alias = "i", visible_alias = "add")]
    Install {
        /// Package names to install (required unless --all or --profile is specified)
        package: Vec<String>,

        /// Install all packages in STAU_DIR (alphabetical order)
        #[arg(short, long)]
        all: bool,

        /// Install the packages listed in a profile under STAU_DIR/profiles
        #[arg(short = 'p', long, value_name = "NAME")]
        profile: Option<String>,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running setup script
        #[arg(long)]
        no_setup: bool,

        /// Print a machine-readable JSON summary instead of human text
        #[arg(long)]
        json: bool,

        #[command(flatten)]
        script: ScriptArgs,
    },

    /// Uninstall packages (removes symlinks and restores original files from dotfiles repo)
    #[command(visible_alias = "u", visible_alias = "rm")]
    Uninstall {
        /// Package names to uninstall (required unless --all or --profile is specified)
        package: Vec<String>,

        /// Uninstall all packages in STAU_DIR (alphabetical order)
        #[arg(short, long)]
        all: bool,

        /// Uninstall the packages listed in a profile under STAU_DIR/profiles
        #[arg(short = 'p', long, value_name = "NAME")]
        profile: Option<String>,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running teardown script
        #[arg(long)]
        no_teardown: bool,

        /// Print a machine-readable JSON summary instead of human text
        #[arg(long)]
        json: bool,

        #[command(flatten)]
        script: ScriptArgs,
    },

    /// Restow packages (uninstall and reinstall, without copying files back)
    #[command(visible_alias = "r")]
    Restow {
        /// Package names to restow (required unless --all or --profile is specified)
        package: Vec<String>,

        /// Restow all packages in STAU_DIR (alphabetical order)
        #[arg(short, long)]
        all: bool,

        /// Restow the packages listed in a profile under STAU_DIR/profiles
        #[arg(short = 'p', long, value_name = "NAME")]
        profile: Option<String>,

        /// Target directory (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Skip running setup script
        #[arg(long)]
        no_setup: bool,

        /// Skip running teardown script
        #[arg(long)]
        no_teardown: bool,

        /// Print a machine-readable JSON summary instead of human text
        #[arg(long)]
        json: bool,

        #[command(flatten)]
        script: ScriptArgs,
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

        /// Show the link count for every package, not only the ones needing attention
        #[arg(short = 'l', long)]
        long: bool,
    },

    /// Show detailed status for one or more packages
    #[command(visible_alias = "s")]
    Status {
        /// Package names to show status for (all packages with --json)
        #[arg(required_unless_present = "json")]
        package: Vec<String>,

        /// Target directory to check status (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Print a machine-readable JSON status envelope
        #[arg(long)]
        json: bool,
    },

    /// Verify that every file of every named package is in the expected state
    #[command(visible_alias = "v")]
    Verify {
        /// Package names to verify
        #[arg(required = true)]
        package: Vec<String>,

        /// The state every file must be in
        #[arg(long, value_enum, default_value = "installed")]
        expect: Expect,

        /// Target directory to check (default: $HOME or $STAU_TARGET)
        #[arg(short, long, env = "STAU_TARGET")]
        target: Option<PathBuf>,

        /// Print mismatches as a machine-readable JSON object
        #[arg(long)]
        json: bool,
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
