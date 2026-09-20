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

use cli::Cli;
use config::Config;
use error::Result;

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

    commands::dispatch(&config, cli.command, cli.dry_run, cli.verbose)
}
