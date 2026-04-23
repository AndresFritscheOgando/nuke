mod cli;
mod nuke;
mod trash;

use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    // Subcommand dispatch — stubs until commands module is added in later tasks
    if cli.command.is_some() {
        eprintln!("Subcommands not yet implemented.");
        std::process::exit(1);
    }

    let scope = cli.scope();
    let force = cli.force;
    let target = cli
        .targets
        .into_iter()
        .next()
        .map(Ok)
        .unwrap_or_else(|| std::env::current_dir().context("failed to get current directory"))?;

    let config = nuke::NukeConfig {
        target,
        scope,
        force,
    };

    nuke::run(config)
}
