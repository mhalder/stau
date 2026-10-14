use clap::Parser;
use std::process;

mod cli;
mod commands;
mod config;
mod error;
mod options;
mod package;
mod profile;
mod script;
mod symlink;
mod template;

use cli::{Cli, Commands};
use config::Config;
use error::Result;

/// Restores the default `SIGPIPE` disposition.
///
/// The Rust runtime sets `SIGPIPE` to `SIG_IGN`, which turns a closed pipe
/// (`stau status pkg | head`) into a write error that `println!` and the
/// completion generator unwrap into a panic and exit code 101. With the
/// default disposition the process is terminated by the signal instead, the
/// way every other Unix command behaves.
fn restore_sigpipe() {
    // SAFETY: called before any thread is spawned, and SIG_DFL is the
    // disposition the process would have had without the Rust runtime.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn main() {
    restore_sigpipe();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);

        // Use appropriate exit code based on error type
        let exit_code = e.exit_code();

        process::exit(exit_code);
    }
}

fn run(cli: Cli) -> Result<()> {
    // Completion scripts must be printable before any dotfiles directory
    // exists, so this runs before Config::new(), which fails without STAU_DIR.
    if let Commands::Completion { shell } = cli.command {
        commands::print_completion(shell, &mut std::io::stdout());
        return Ok(());
    }

    let config = Config::new()?;

    if cli.verbose {
        println!("STAU_DIR: {}", config.stau_dir.display());
    }

    commands::dispatch(&config, cli.command, cli.dry_run, cli.verbose)
}
