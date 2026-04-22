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

    let target = match cli.target.clone() {
        Some(p) => p,
        None => std::env::current_dir().context("failed to get current directory")?,
    };

    let config = nuke::NukeConfig {
        target,
        scope: cli.scope(),
        force: cli.force,
    };

    nuke::run(config)
}
