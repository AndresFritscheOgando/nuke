use anyhow::{Context, Result};
use colored::Colorize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cli::Scope;
use crate::trash::Trash;

pub struct NukeConfig {
    pub targets: Vec<PathBuf>,
    pub scope: Scope,
    pub force: bool,
    pub dry_run: bool,
    pub pattern: Option<String>,
    pub exclude: Vec<String>,
}

pub fn run(config: NukeConfig) -> Result<()> {
    for target in &config.targets {
        validate_target(target)?;
    }

    let include_pattern = config
        .pattern
        .as_deref()
        .map(|p| {
            glob::Pattern::new(p)
                .with_context(|| format!("invalid --pattern glob: {}", p))
        })
        .transpose()?;

    let exclude_patterns = config
        .exclude
        .iter()
        .map(|p| {
            glob::Pattern::new(p)
                .with_context(|| format!("invalid --exclude glob: {}", p))
        })
        .collect::<Result<Vec<_>>>()?;

    let trash = Trash::new()?;

    let mut all_items: Vec<(PathBuf, Arc<String>)> = Vec::new();
    let mut target_counts: Vec<(PathBuf, usize)> = Vec::new();

    for target in &config.targets {
        let target_name = Arc::new(
            target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| target.to_string_lossy().into_owned()),
        );
        let items = collect_items(target, &config.scope, &include_pattern, &exclude_patterns)?;
        let count = items.len();
        for item in items {
            all_items.push((item, Arc::clone(&target_name)));
        }
        target_counts.push((target.clone(), count));
    }

    if all_items.is_empty() {
        println!("{}", "Nothing to nuke.".yellow());
        return Ok(());
    }

    let total = all_items.len();
    preview(&config.scope, &trash, &target_counts, total, config.dry_run);

    if config.dry_run {
        for (item, target_name) in &all_items {
            println!("  [{}] would move: {}", target_name, item.display());
        }
        return Ok(());
    }

    if !config.force {
        confirm()?;
    }

    trash.create()?;
    let mut errors = 0usize;
    for (item, target_name) in &all_items {
        if let Err(e) = trash.send_to_namespace(item, &target_name) {
            eprintln!("{} {}", "error:".red().bold(), e);
            errors += 1;
        }
    }

    let moved = total - errors;
    let suffix = if errors > 0 {
        format!(" ({} failed)", errors)
    } else {
        String::new()
    };
    println!(
        "{} {} item(s) moved to {}{}",
        "Done.".green().bold(),
        moved,
        trash.path.display(),
        suffix
    );

    Ok(())
}

fn validate_target(target: &Path) -> Result<()> {
    if !target.exists() {
        anyhow::bail!("target '{}' does not exist", target.display());
    }
    if !target.is_dir() {
        anyhow::bail!("target '{}' is not a directory", target.display());
    }

    let canonical = target
        .canonicalize()
        .with_context(|| format!("failed to resolve '{}'", target.display()))?;

    if canonical == Path::new("/") {
        anyhow::bail!("refusing to nuke '/' — this would be catastrophic");
    }

    if let Ok(home_str) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        if let Ok(home_canonical) = PathBuf::from(home_str).canonicalize() {
            if canonical == home_canonical {
                anyhow::bail!("refusing to nuke your home directory");
            }
        }
    }

    Ok(())
}

fn collect_items(
    target: &Path,
    scope: &Scope,
    include_pattern: &Option<glob::Pattern>,
    exclude_patterns: &[glob::Pattern],
) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(target)
        .with_context(|| format!("failed to read directory '{}'", target.display()))?;

    let mut items = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", target.display()))?;
        let path = entry.path();

        if matches!(scope, Scope::FilesOnly) && path.is_dir() {
            continue;
        }

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if let Some(pat) = include_pattern {
            if !pat.matches(name) {
                continue;
            }
        }

        if exclude_patterns.iter().any(|p| p.matches(name)) {
            continue;
        }

        items.push(path);
    }
    Ok(items)
}

fn preview(
    scope: &Scope,
    trash: &Trash,
    target_counts: &[(PathBuf, usize)],
    total: usize,
    dry_run: bool,
) {
    let scope_label = match scope {
        Scope::FilesOnly => "files only",
        Scope::All => "files + directories",
    };
    let header = if dry_run {
        "--- nuke preview (dry run) ---"
    } else {
        "--- nuke preview ---"
    };
    println!("{}", header.bold());
    for (target, count) in target_counts {
        println!("  Target : {} ({} items)", target.display(), count);
    }
    println!("  Scope  : {}", scope_label);
    if !dry_run {
        println!("  Trash  : {}", trash.path.display());
    }
    println!("  Total  : {}", total);
    let border = "-".repeat(header.len());
    println!("{}", border.bold());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_items_no_filter() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), b"a").unwrap();
        fs::write(dir.path().join("b.rs"), b"b").unwrap();

        let items = collect_items(dir.path(), &Scope::FilesOnly, &None, &[]).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_collect_items_pattern_includes_only_matches() {
        let pat = glob::Pattern::new("*.log").unwrap();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("foo.log"), b"log").unwrap();
        fs::write(dir.path().join("bar.rs"), b"code").unwrap();

        let items = collect_items(dir.path(), &Scope::FilesOnly, &Some(pat), &[]).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name().unwrap(), "foo.log");
    }

    #[test]
    fn test_collect_items_exclude_removes_matches() {
        let excl = vec![glob::Pattern::new("*.log").unwrap()];
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("keep.txt"), b"keep").unwrap();
        fs::write(dir.path().join("drop.log"), b"drop").unwrap();

        let items = collect_items(dir.path(), &Scope::FilesOnly, &None, &excl).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name().unwrap(), "keep.txt");
    }

    #[test]
    fn test_collect_items_exclude_beats_pattern() {
        let pat = glob::Pattern::new("*.log").unwrap();
        let excl = vec![glob::Pattern::new("*.log").unwrap()];
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("foo.log"), b"log").unwrap();

        let items = collect_items(dir.path(), &Scope::FilesOnly, &Some(pat), &excl).unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_collect_items_files_only_skips_dirs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let items = collect_items(dir.path(), &Scope::FilesOnly, &None, &[]).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name().unwrap(), "file.txt");
    }

    #[test]
    fn test_collect_items_all_includes_dirs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let items = collect_items(dir.path(), &Scope::All, &None, &[]).unwrap();
        assert_eq!(items.len(), 2);
    }
}
