mod cli;
mod commands;
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

    match cli.command {
        Some(cli::Command::List) => commands::list::run(),
        Some(cli::Command::Restore) => commands::restore::run(),
        Some(cli::Command::Empty(args)) => commands::empty::run(args.all),
        None => {
            let scope = cli.scope();
            let force = cli.force;
            let dry_run = cli.dry_run;
            let pattern = cli.pattern;
            let exclude = cli.exclude;
            let targets = if cli.targets.is_empty() {
                vec![std::env::current_dir().context("failed to get current directory")?]
            } else {
                cli.targets
            };

            let config = nuke::NukeConfig {
                targets,
                scope,
                force,
                dry_run,
                pattern,
                exclude,
            };

            nuke::run(config)
        }
    }
}
