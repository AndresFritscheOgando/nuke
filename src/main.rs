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

    // Subcommand dispatch — stubs until commands module added in next task
    if cli.command.is_some() {
        eprintln!("Subcommands not yet implemented.");
        std::process::exit(1);
    }

    let targets = if cli.targets.is_empty() {
        vec![std::env::current_dir().context("failed to get current directory")?]
    } else {
        cli.targets.clone()
    };

    let config = nuke::NukeConfig {
        targets,
        scope: cli.scope(),
        force: cli.force,
        dry_run: cli.dry_run,
        pattern: cli.pattern.clone(),
        exclude: cli.exclude.clone(),
    };

    nuke::run(config)
}
