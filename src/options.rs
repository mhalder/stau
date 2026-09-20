use crate::script::ScriptOptions;
use std::path::PathBuf;

/// Options shared by every package-mutating command (install, uninstall, restow).
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    /// Target directory override.
    pub target: Option<PathBuf>,
    /// Print a machine-readable JSON summary instead of human text.
    pub json: bool,
    /// Show what would be done without making changes.
    pub dry_run: bool,
    /// Verbose output.
    pub verbose: bool,
    /// Hook script execution options.
    pub script: ScriptOptions,
}
