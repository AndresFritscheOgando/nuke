use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::Scope;
use crate::trash::Trash;

/// Configuration for a single nuke operation.
pub struct NukeConfig {
    pub target: PathBuf,
    pub scope: Scope,
    pub force: bool,
}

/// Runs a nuke operation: validate → collect → preview → confirm → trash.
pub fn run(config: NukeConfig) -> Result<()> {
    validate_target(&config.target)?;

    let trash = Trash::new()?;
    let items = collect_items(&config.target, &config.scope)?;

    if items.is_empty() {
        println!("{}", "Nothing to nuke.".yellow());
        return Ok(());
    }

    preview(&config.target, &config.scope, &trash, items.len());

    if !config.force {
        confirm()?;
    }

    trash.create()?;
    for item in &items {
        trash.send(item)?;
    }

    println!(
        "{} {} item(s) moved to {}",
        "Done.".green().bold(),
        items.len(),
        trash.path.display()
    );

    Ok(())
}

fn validate_target(target: &Path) -> Result<()> {
    if !target.exists() {
        bail!("target '{}' does not exist", target.display());
    }
    if !target.is_dir() {
        bail!("target '{}' is not a directory", target.display());
    }

    let canonical = target
        .canonicalize()
        .with_context(|| format!("failed to resolve '{}'", target.display()))?;

    if canonical == Path::new("/") {
        bail!("refusing to nuke '/' — this would be catastrophic");
    }

    if let Ok(home_str) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        if let Ok(home_canonical) = PathBuf::from(home_str).canonicalize() {
            if canonical == home_canonical {
                bail!("refusing to nuke your home directory");
            }
        }
    }

    Ok(())
}

fn collect_items(target: &Path, scope: &Scope) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(target)
        .with_context(|| format!("failed to read directory '{}'", target.display()))?;

    let mut items = Vec::new();
    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read entry in '{}'", target.display()))?;
        let path = entry.path();
        if matches!(scope, Scope::FilesOnly) && path.is_dir() {
            continue;
        }
        items.push(path);
    }
    Ok(items)
}

fn preview(target: &Path, scope: &Scope, trash: &Trash, count: usize) {
    let scope_label = match scope {
        Scope::FilesOnly => "files only",
        Scope::All => "files + directories",
    };
    println!("{}", "--- nuke preview ---".bold());
    println!("  Target : {}", target.display());
    println!("  Scope  : {}", scope_label);
    println!("  Trash  : {}", trash.path.display());
    println!("  Items  : {}", count);
    println!("{}", "--------------------".bold());
}

fn confirm() -> Result<()> {
    print!("Proceed? {} ", "[y/N]".yellow());
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation input")?;

    if input.trim().eq_ignore_ascii_case("y") {
        Ok(())
    } else {
        println!("Aborted.");
        std::process::exit(0);
    }
}
