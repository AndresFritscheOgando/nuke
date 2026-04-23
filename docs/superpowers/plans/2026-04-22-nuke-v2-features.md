# nuke v2 Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add trash management subcommands (`list`, `restore`, `empty`), filtering (`--pattern`, `--exclude`), multi-target (`-t` repeatable), and `--dry-run` to make `nuke` a general-purpose `rm` replacement.

**Architecture:** CLI restructures to an optional subcommand enum (git-style); the default path is the existing nuke pipeline extended with multi-target, filtering, and dry-run. Trash management lives in three new command modules backed by new functions in `trash.rs`. Items are namespaced under `~/.nuke-trash/<timestamp>/<target-dir-name>/` to prevent collisions across multi-target runs. Because this is a binary crate, every task must leave `main.rs` compiling — tasks that touch the public API of `cli.rs` or `nuke.rs` also update `main.rs` in the same commit.

**Tech Stack:** Rust, clap 4 (derive), anyhow, colored, chrono, walkdir, glob 0.3, dialoguer 0.11, bytesize 1.3, tempfile 3 (dev)

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add glob, dialoguer, bytesize; add tempfile dev-dep |
| Modify | `src/cli.rs` | Clap enum with optional subcommand; extend NukeArgs |
| Modify | `src/main.rs` | Dispatch subcommands (updated incrementally across tasks) |
| Modify | `src/nuke.rs` | Multi-target, dry-run, filtering pipeline |
| Modify | `src/trash.rs` | TrashSession, list/restore/empty functions, send_to_namespace |
| Create | `src/commands/mod.rs` | Module declarations |
| Create | `src/commands/list.rs` | Format session table |
| Create | `src/commands/restore.rs` | Interactive session picker + restore |
| Create | `src/commands/empty.rs` | Interactive session picker + permanent delete |

---

## Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update Cargo.toml**

```toml
[package]
name = "nuke"
version = "2.0.0"
edition = "2021"
description = "Safely move directory contents to a timestamped trash folder"
license = "MIT"

[dependencies]
clap = { version = "=4.4.18", features = ["derive"] }
anyhow = "=1.0.86"
chrono = "=0.4.38"
colored = "=2.1.0"
walkdir = "=2.5.0"
glob = "=0.3.1"
dialoguer = "=0.11.0"
bytesize = "=1.3.0"

[dev-dependencies]
tempfile = "=3.10.1"
```

- [ ] **Step 2: Fetch and verify**

```bash
cargo fetch && cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s)`

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add glob, dialoguer, bytesize; add tempfile dev-dep"
```

---

## Task 2: Restructure CLI and extend trash.rs session management

Both `cli.rs` and `trash.rs` change here. `main.rs` is updated in the same commit to keep compilation green. `nuke.rs` is unchanged in this task.

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/trash.rs`
- Modify: `src/main.rs` (temporary stub — keeps binary compiling)

- [ ] **Step 1: Write failing CLI parse tests (append to src/cli.rs)**

Add at the bottom of the current `src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_default_nuke_flags() {
        let cli = Cli::parse_from(["nuke", "--force", "--dry-run"]);
        assert!(cli.force);
        assert!(cli.dry_run);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_multiple_targets() {
        let cli = Cli::parse_from(["nuke", "-t", "/tmp/a", "-t", "/tmp/b"]);
        assert_eq!(cli.targets.len(), 2);
    }

    #[test]
    fn test_parse_pattern_and_exclude() {
        let cli = Cli::parse_from([
            "nuke", "--pattern", "*.log", "--exclude", "keep.log", "--exclude", "audit.log",
        ]);
        assert_eq!(cli.pattern.as_deref(), Some("*.log"));
        assert_eq!(cli.exclude.len(), 2);
    }

    #[test]
    fn test_parse_list_subcommand() {
        let cli = Cli::parse_from(["nuke", "list"]);
        assert!(matches!(cli.command, Some(Command::List)));
    }

    #[test]
    fn test_parse_restore_subcommand() {
        let cli = Cli::parse_from(["nuke", "restore"]);
        assert!(matches!(cli.command, Some(Command::Restore)));
    }

    #[test]
    fn test_parse_empty_subcommand() {
        let cli = Cli::parse_from(["nuke", "empty"]);
        match &cli.command {
            Some(Command::Empty(args)) => assert!(!args.all),
            _ => panic!("expected Empty subcommand"),
        }
    }

    #[test]
    fn test_parse_empty_all_flag() {
        let cli = Cli::parse_from(["nuke", "empty", "--all"]);
        match &cli.command {
            Some(Command::Empty(args)) => assert!(args.all),
            _ => panic!("expected Empty subcommand"),
        }
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test 2>&1 | tail -10
```

Expected: compile errors — `Cli` doesn't have `targets`, `dry_run`, `pattern`, `exclude`, `command` yet.

- [ ] **Step 3: Write failing trash session tests (append to src/trash.rs)**

Add at the bottom of the current `src/trash.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_session(root: &Path, timestamp: &str, items: &[(&str, &[u8])]) -> PathBuf {
        let session = root.join(timestamp);
        let ns = session.join("target");
        fs::create_dir_all(&ns).unwrap();
        for (name, content) in items {
            fs::write(ns.join(name), content).unwrap();
        }
        session
    }

    #[test]
    fn test_list_sessions_nonexistent_root_returns_empty() {
        let root = TempDir::new().unwrap();
        let sessions = list_sessions_in(&root.path().join("nonexistent")).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_sessions_sorted_newest_first() {
        let root = TempDir::new().unwrap();
        make_session(root.path(), "2026-01-01_00-00-00", &[("f.txt", b"x")]);
        make_session(root.path(), "2026-03-01_00-00-00", &[("g.txt", b"y")]);

        let sessions = list_sessions_in(root.path()).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].timestamp, "2026-03-01_00-00-00");
        assert_eq!(sessions[1].timestamp, "2026-01-01_00-00-00");
    }

    #[test]
    fn test_restore_session_moves_items_to_dest() {
        let trash_root = TempDir::new().unwrap();
        let session_path =
            make_session(trash_root.path(), "2026-01-01_00-00-00", &[("file.txt", b"hello")]);
        let dest = TempDir::new().unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path.clone(),
            item_count: 1,
            total_size: 5,
        };

        restore_session(&session, dest.path()).unwrap();

        assert!(dest.path().join("target").join("file.txt").exists());
        assert!(!session_path.exists());
    }

    #[test]
    fn test_restore_session_aborts_on_conflict() {
        let trash_root = TempDir::new().unwrap();
        let session_path =
            make_session(trash_root.path(), "2026-01-01_00-00-00", &[("file.txt", b"x")]);
        let dest = TempDir::new().unwrap();
        fs::create_dir(dest.path().join("target")).unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path,
            item_count: 1,
            total_size: 1,
        };

        let result = restore_session(&session, dest.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("conflicts at destination"));
    }

    #[test]
    fn test_empty_session_removes_dir() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("2026-01-01_00-00-00");
        fs::create_dir_all(&session_path).unwrap();
        fs::write(session_path.join("file.txt"), b"data").unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path.clone(),
            item_count: 1,
            total_size: 4,
        };

        empty_session(&session).unwrap();
        assert!(!session_path.exists());
    }

    #[test]
    fn test_measure_session_counts_files_and_size() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        fs::write(dir.path().join("b.txt"), b"world!").unwrap();

        let (count, size) = measure_session(dir.path()).unwrap();
        assert_eq!(count, 2);
        assert_eq!(size, 11);
    }
}
```

- [ ] **Step 4: Replace src/cli.rs with new implementation**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    FilesOnly,
    All,
}

#[derive(Parser, Debug)]
#[command(
    name = "nuke",
    about = "Safely move directory contents to a timestamped trash folder"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Nuke files + subdirectories
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Nuke files only (default behavior)
    #[arg(long = "files-only")]
    pub files_only: bool,

    /// Target directory (repeatable; default: cwd)
    #[arg(short = 't', long = "target")]
    pub targets: Vec<PathBuf>,

    /// Skip confirmation prompt
    #[arg(long = "force")]
    pub force: bool,

    /// Show what would be moved without moving anything
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Only nuke items matching glob (e.g. "*.log")
    #[arg(long = "pattern")]
    pub pattern: Option<String>,

    /// Exclude items matching glob (repeatable)
    #[arg(long = "exclude")]
    pub exclude: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List all trash sessions
    List,
    /// Interactively restore a trash session
    Restore,
    /// Permanently delete trash sessions
    Empty(EmptyArgs),
}

#[derive(Parser, Debug)]
pub struct EmptyArgs {
    /// Delete all sessions without interactive selection
    #[arg(long = "all")]
    pub all: bool,
}

impl Cli {
    pub fn scope(&self) -> Scope {
        if self.all {
            Scope::All
        } else {
            Scope::FilesOnly
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_default_nuke_flags() {
        let cli = Cli::parse_from(["nuke", "--force", "--dry-run"]);
        assert!(cli.force);
        assert!(cli.dry_run);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_multiple_targets() {
        let cli = Cli::parse_from(["nuke", "-t", "/tmp/a", "-t", "/tmp/b"]);
        assert_eq!(cli.targets.len(), 2);
    }

    #[test]
    fn test_parse_pattern_and_exclude() {
        let cli = Cli::parse_from([
            "nuke", "--pattern", "*.log", "--exclude", "keep.log", "--exclude", "audit.log",
        ]);
        assert_eq!(cli.pattern.as_deref(), Some("*.log"));
        assert_eq!(cli.exclude.len(), 2);
    }

    #[test]
    fn test_parse_list_subcommand() {
        let cli = Cli::parse_from(["nuke", "list"]);
        assert!(matches!(cli.command, Some(Command::List)));
    }

    #[test]
    fn test_parse_restore_subcommand() {
        let cli = Cli::parse_from(["nuke", "restore"]);
        assert!(matches!(cli.command, Some(Command::Restore)));
    }

    #[test]
    fn test_parse_empty_subcommand() {
        let cli = Cli::parse_from(["nuke", "empty"]);
        match &cli.command {
            Some(Command::Empty(args)) => assert!(!args.all),
            _ => panic!("expected Empty subcommand"),
        }
    }

    #[test]
    fn test_parse_empty_all_flag() {
        let cli = Cli::parse_from(["nuke", "empty", "--all"]);
        match &cli.command {
            Some(Command::Empty(args)) => assert!(args.all),
            _ => panic!("expected Empty subcommand"),
        }
    }
}
```

- [ ] **Step 5: Replace src/trash.rs with new implementation**

```rust
use anyhow::{bail, Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Trash {
    pub path: PathBuf,
}

pub struct TrashSession {
    pub timestamp: String,
    pub path: PathBuf,
    pub item_count: usize,
    pub total_size: u64,
}

impl Trash {
    /// Resolves `~/.nuke-trash/<YYYY-MM-DD_HH-MM-SS>/`.
    pub fn new() -> Result<Self> {
        let home = home_dir()?;
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let path = home.join(".nuke-trash").join(timestamp);
        Ok(Self { path })
    }

    /// Creates the trash directory on disk.
    pub fn create(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)
            .with_context(|| format!("failed to create trash dir '{}'", self.path.display()))
    }

    /// Moves an item into a named namespace subdirectory of this session.
    pub fn send_to_namespace(&self, item: &Path, namespace: &str) -> Result<()> {
        let ns_dir = self.path.join(namespace);
        std::fs::create_dir_all(&ns_dir)
            .with_context(|| format!("failed to create namespace dir '{}'", ns_dir.display()))?;
        let name = item
            .file_name()
            .with_context(|| format!("item '{}' has no file name", item.display()))?;
        let dest = ns_dir.join(name);
        move_item(item, &dest)
    }
}

/// Returns all trash sessions sorted newest-first, reading from `~/.nuke-trash/`.
pub fn list_sessions() -> Result<Vec<TrashSession>> {
    let home = home_dir()?;
    list_sessions_in(&home.join(".nuke-trash"))
}

/// Returns all trash sessions sorted newest-first from the given root path.
pub fn list_sessions_in(trash_root: &Path) -> Result<Vec<TrashSession>> {
    if !trash_root.exists() {
        return Ok(vec![]);
    }
    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(trash_root)
        .with_context(|| format!("failed to read trash directory '{}'", trash_root.display()))?
    {
        let entry = entry.context("failed to read trash entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let timestamp = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let (item_count, total_size) = measure_session(&path)?;
        sessions.push(TrashSession {
            timestamp,
            path,
            item_count,
            total_size,
        });
    }
    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(sessions)
}

/// Moves all top-level entries from a session to `dest`. Aborts if any name conflicts exist.
/// Removes the session directory on success.
pub fn restore_session(session: &TrashSession, dest: &Path) -> Result<()> {
    let mut conflicts = Vec::new();
    for entry in std::fs::read_dir(&session.path)
        .with_context(|| format!("failed to read session '{}'", session.path.display()))?
    {
        let entry = entry.context("failed to read session entry")?;
        if dest.join(entry.file_name()).exists() {
            conflicts.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    if !conflicts.is_empty() {
        bail!(
            "conflicts at destination — resolve before restoring:\n  {}",
            conflicts.join("\n  ")
        );
    }

    for entry in std::fs::read_dir(&session.path)
        .with_context(|| format!("failed to read session '{}'", session.path.display()))?
    {
        let entry = entry.context("failed to read session entry")?;
        move_item(&entry.path(), &dest.join(entry.file_name()))?;
    }

    std::fs::remove_dir_all(&session.path)
        .with_context(|| format!("failed to remove session dir '{}'", session.path.display()))
}

/// Permanently deletes a single trash session.
pub fn empty_session(session: &TrashSession) -> Result<()> {
    std::fs::remove_dir_all(&session.path)
        .with_context(|| format!("failed to empty session '{}'", session.path.display()))
}

/// Permanently deletes all trash sessions. Returns count deleted.
pub fn empty_all() -> Result<usize> {
    let sessions = list_sessions()?;
    let count = sessions.len();
    for session in &sessions {
        empty_session(session)?;
    }
    Ok(count)
}

/// Counts files and sums bytes recursively under a session directory.
pub fn measure_session(path: &Path) -> Result<(usize, u64)> {
    let mut count = 0usize;
    let mut size = 0u64;
    for entry in WalkDir::new(path).min_depth(1) {
        let entry = entry.with_context(|| format!("failed to walk '{}'", path.display()))?;
        if entry.file_type().is_file() {
            count += 1;
            size += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok((count, size))
}

fn move_item(src: &Path, dest: &Path) -> Result<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        // EXDEV (18): cross-device rename — fall back to copy then remove.
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_all(src, dest)?;
            remove_all(src)
        }
        Err(e) => Err(e).with_context(|| format!("failed to move '{}' to trash", src.display())),
    }
}

fn copy_all(src: &Path, dest: &Path) -> Result<()> {
    if src.is_file() {
        std::fs::copy(src, dest).with_context(|| {
            format!("failed to copy '{}' to '{}'", src.display(), dest.display())
        })?;
        return Ok(());
    }

    std::fs::create_dir_all(dest)
        .with_context(|| format!("failed to create directory '{}'", dest.display()))?;

    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry.with_context(|| format!("failed to walk '{}'", src.display()))?;
        let relative = entry
            .path()
            .strip_prefix(src)
            .context("walkdir path not relative to source")?;
        let target = dest.join(relative);

        if entry.path().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("failed to create directory '{}'", target.display()))?;
        } else {
            std::fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "failed to copy '{}' to '{}'",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn remove_all(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory '{}'", path.display()))
    } else {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove file '{}'", path.display()))
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("could not determine home directory ($HOME or $USERPROFILE not set)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_session(root: &Path, timestamp: &str, items: &[(&str, &[u8])]) -> PathBuf {
        let session = root.join(timestamp);
        let ns = session.join("target");
        fs::create_dir_all(&ns).unwrap();
        for (name, content) in items {
            fs::write(ns.join(name), content).unwrap();
        }
        session
    }

    #[test]
    fn test_list_sessions_nonexistent_root_returns_empty() {
        let root = TempDir::new().unwrap();
        let sessions = list_sessions_in(&root.path().join("nonexistent")).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_sessions_sorted_newest_first() {
        let root = TempDir::new().unwrap();
        make_session(root.path(), "2026-01-01_00-00-00", &[("f.txt", b"x")]);
        make_session(root.path(), "2026-03-01_00-00-00", &[("g.txt", b"y")]);

        let sessions = list_sessions_in(root.path()).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].timestamp, "2026-03-01_00-00-00");
        assert_eq!(sessions[1].timestamp, "2026-01-01_00-00-00");
    }

    #[test]
    fn test_restore_session_moves_items_to_dest() {
        let trash_root = TempDir::new().unwrap();
        let session_path =
            make_session(trash_root.path(), "2026-01-01_00-00-00", &[("file.txt", b"hello")]);
        let dest = TempDir::new().unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path.clone(),
            item_count: 1,
            total_size: 5,
        };

        restore_session(&session, dest.path()).unwrap();

        assert!(dest.path().join("target").join("file.txt").exists());
        assert!(!session_path.exists());
    }

    #[test]
    fn test_restore_session_aborts_on_conflict() {
        let trash_root = TempDir::new().unwrap();
        let session_path =
            make_session(trash_root.path(), "2026-01-01_00-00-00", &[("file.txt", b"x")]);
        let dest = TempDir::new().unwrap();
        fs::create_dir(dest.path().join("target")).unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path,
            item_count: 1,
            total_size: 1,
        };

        let result = restore_session(&session, dest.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("conflicts at destination"));
    }

    #[test]
    fn test_empty_session_removes_dir() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("2026-01-01_00-00-00");
        fs::create_dir_all(&session_path).unwrap();
        fs::write(session_path.join("file.txt"), b"data").unwrap();

        let session = TrashSession {
            timestamp: "2026-01-01_00-00-00".into(),
            path: session_path.clone(),
            item_count: 1,
            total_size: 4,
        };

        empty_session(&session).unwrap();
        assert!(!session_path.exists());
    }

    #[test]
    fn test_measure_session_counts_files_and_size() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        fs::write(dir.path().join("b.txt"), b"world!").unwrap();

        let (count, size) = measure_session(dir.path()).unwrap();
        assert_eq!(count, 2);
        assert_eq!(size, 11);
    }
}
```

- [ ] **Step 6: Update src/main.rs to compile with new cli.rs (temporary stub)**

`main.rs` still calls the old `NukeConfig` API. Update it to use `cli.targets` but keep `NukeConfig` the same shape so everything compiles. Replace `src/main.rs` with:

```rust
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

    let target = cli
        .targets
        .into_iter()
        .next()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let config = nuke::NukeConfig {
        target,
        scope: cli.scope(),
        force: cli.force,
    };

    nuke::run(config)
}
```

Note: `nuke.rs` still has the old `NukeConfig { target, scope, force }` signature. This stub bridges the gap until Task 3.

- [ ] **Step 7: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: CLI tests pass (7), trash tests pass (5). The binary compiles.

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/trash.rs src/main.rs
git commit -m "feat(cli,trash): subcommand enum, TrashSession, session management functions"
```

---

## Task 3: Update nuke.rs for multi-target, dry-run, and filtering

This task also updates `main.rs` to use the new `NukeConfig` — both go in the same commit to keep the binary compiling.

**Files:**
- Modify: `src/nuke.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing filtering tests (append to src/nuke.rs)**

Add at the bottom of the current `src/nuke.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to confirm collect_items tests fail**

```bash
cargo test 2>&1 | grep -E "FAILED|error\[" | head -10
```

Expected: compile errors — `collect_items` doesn't accept pattern/exclude args yet.

- [ ] **Step 3: Replace src/nuke.rs with new implementation**

```rust
use anyhow::{Context, Result};
use colored::Colorize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    let mut all_items: Vec<(PathBuf, String)> = Vec::new();
    let mut target_counts: Vec<(PathBuf, usize)> = Vec::new();

    for target in &config.targets {
        let target_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let items = collect_items(target, &config.scope, &include_pattern, &exclude_patterns)?;
        let count = items.len();
        for item in items {
            all_items.push((item, target_name.clone()));
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
        for (item, _) in &all_items {
            println!("  would move: {}", item.display());
        }
        return Ok(());
    }

    if !config.force {
        confirm()?;
    }

    trash.create()?;
    let mut errors = 0usize;
    for (item, target_name) in &all_items {
        if let Err(e) = trash.send_to_namespace(item, target_name) {
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
    let border = if dry_run {
        "------------------------------"
    } else {
        "--------------------"
    };
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
```

- [ ] **Step 4: Update src/main.rs to use new NukeConfig**

Replace `src/main.rs` with the full implementation. Commands module is still a stub (`eprintln!` + exit), added in the next task.

```rust
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
```

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all 18 tests pass (7 CLI + 5 trash + 6 nuke).

- [ ] **Step 6: Commit**

```bash
git add src/nuke.rs src/main.rs
git commit -m "feat(nuke): multi-target, dry-run, pattern/exclude filtering"
```

---

## Task 4: Add commands module with list, restore, empty

**Files:**
- Create: `src/commands/mod.rs`
- Create: `src/commands/list.rs`
- Create: `src/commands/restore.rs`
- Create: `src/commands/empty.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create src/commands/mod.rs**

```rust
pub mod empty;
pub mod list;
pub mod restore;
```

- [ ] **Step 2: Create src/commands/list.rs**

```rust
use anyhow::Result;
use bytesize::ByteSize;
use colored::Colorize;

use crate::trash::list_sessions;

pub fn run() -> Result<()> {
    let sessions = list_sessions()?;

    if sessions.is_empty() {
        println!("{}", "No trash sessions found.".yellow());
        return Ok(());
    }

    println!("{}", "--- trash sessions ---".bold());
    println!("{:<25} {:>8} {:>12}", "timestamp", "files", "size");
    println!("{}", "-".repeat(47));
    for session in &sessions {
        println!(
            "{:<25} {:>8} {:>12}",
            session.timestamp,
            session.item_count,
            ByteSize(session.total_size).to_string(),
        );
    }
    println!("{}", "-".repeat(47));
    println!("total: {} session(s)", sessions.len());

    Ok(())
}
```

- [ ] **Step 3: Create src/commands/restore.rs**

```rust
use anyhow::{bail, Context, Result};
use bytesize::ByteSize;
use colored::Colorize;
use dialoguer::{Input, Select};
use std::path::PathBuf;

use crate::trash::{list_sessions, restore_session};

pub fn run() -> Result<()> {
    let sessions = list_sessions()?;

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return Ok(());
    }

    let labels: Vec<String> = sessions
        .iter()
        .map(|s| {
            format!(
                "{} — {} files, {}",
                s.timestamp,
                s.item_count,
                ByteSize(s.total_size)
            )
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select session to restore")
        .items(&labels)
        .default(0)
        .interact()
        .context("failed to show session picker")?;

    let session = &sessions[selection];

    let dest_str: String = Input::new()
        .with_prompt("Restore destination (blank = cwd)")
        .allow_empty(true)
        .interact_text()
        .context("failed to read destination")?;

    let dest = if dest_str.trim().is_empty() {
        std::env::current_dir().context("failed to get current directory")?
    } else {
        PathBuf::from(dest_str.trim())
    };

    if !dest.exists() {
        bail!("destination '{}' does not exist", dest.display());
    }
    if !dest.is_dir() {
        bail!("destination '{}' is not a directory", dest.display());
    }

    restore_session(session, &dest)?;

    println!(
        "{} session '{}' restored to {}",
        "Done.".green().bold(),
        session.timestamp,
        dest.display()
    );

    Ok(())
}
```

- [ ] **Step 4: Create src/commands/empty.rs**

```rust
use anyhow::{Context, Result};
use bytesize::ByteSize;
use colored::Colorize;
use dialoguer::{Confirm, MultiSelect};

use crate::trash::{empty_all, empty_session, list_sessions};

pub fn run(all: bool) -> Result<()> {
    let sessions = list_sessions()?;

    if sessions.is_empty() {
        println!("{}", "No trash sessions found.".yellow());
        return Ok(());
    }

    if all {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Permanently delete {} session(s)?",
                sessions.len()
            ))
            .default(false)
            .interact()
            .context("failed to read confirmation")?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }

        let count = empty_all()?;
        println!(
            "{} {} session(s) permanently deleted.",
            "Done.".green().bold(),
            count
        );
        return Ok(());
    }

    let labels: Vec<String> = sessions
        .iter()
        .map(|s| {
            format!(
                "{} — {} files, {}",
                s.timestamp,
                s.item_count,
                ByteSize(s.total_size)
            )
        })
        .collect();

    let selections = MultiSelect::new()
        .with_prompt("Select sessions to permanently delete (space to toggle)")
        .items(&labels)
        .interact()
        .context("failed to show session picker")?;

    if selections.is_empty() {
        println!("Nothing selected.");
        return Ok(());
    }

    let count = selections.len();

    let confirmed = Confirm::new()
        .with_prompt(format!("Permanently delete {} selected session(s)?", count))
        .default(false)
        .interact()
        .context("failed to read confirmation")?;

    if !confirmed {
        println!("Aborted.");
        return Ok(());
    }

    for idx in &selections {
        empty_session(&sessions[*idx])?;
    }

    println!(
        "{} {} session(s) permanently deleted.",
        "Done.".green().bold(),
        count
    );

    Ok(())
}
```

- [ ] **Step 5: Update src/main.rs with full subcommand dispatch**

```rust
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
    }
}
```

- [ ] **Step 6: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s)`

- [ ] **Step 7: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all 18 tests pass with 0 failures.

- [ ] **Step 8: Commit**

```bash
git add src/commands/mod.rs src/commands/list.rs src/commands/restore.rs src/commands/empty.rs src/main.rs
git commit -m "feat(commands): add list, restore, empty subcommands"
```

---

## Task 5: Smoke test

**No files modified — manual verification only.**

- [ ] **Step 1: Test default nuke with dry-run**

```bash
mkdir /tmp/nuke-test && touch /tmp/nuke-test/a.txt /tmp/nuke-test/b.log
./target/release/nuke -t /tmp/nuke-test --dry-run
```

Expected output includes:
```
--- nuke preview (dry run) ---
  Target : /tmp/nuke-test (2 items)
  Scope  : files only
  Total  : 2
```

Files in `/tmp/nuke-test` must still exist after this command.

- [ ] **Step 2: Test --pattern filter with dry-run**

```bash
./target/release/nuke -t /tmp/nuke-test --pattern "*.log" --dry-run
```

Expected: only `b.log` listed under "would move". `a.txt` absent.

- [ ] **Step 3: Nuke with --force and verify trash structure**

```bash
./target/release/nuke -t /tmp/nuke-test --force
ls ~/.nuke-trash/
ls ~/.nuke-trash/*/
```

Expected: timestamped session dir containing a `nuke-test/` namespace subdirectory with `a.txt` and `b.log` inside.

- [ ] **Step 4: Test nuke list**

```bash
./target/release/nuke list
```

Expected: table showing the session with file count and size. Example:
```
--- trash sessions ---
timestamp                    files         size
-----------------------------------------------
2026-04-22_10-00-00              2        8.0 B
-----------------------------------------------
total: 1 session(s)
```

- [ ] **Step 5: Test nuke restore (interactive)**

```bash
mkdir /tmp/restore-dest
./target/release/nuke restore
```

Select the session, enter `/tmp/restore-dest` as destination. Verify:
```bash
ls /tmp/restore-dest/nuke-test/
```

Expected: `a.txt` and `b.log` present. `nuke list` shows 0 sessions.

- [ ] **Step 6: Nuke again and test empty --all**

```bash
mkdir /tmp/nuke-test2 && touch /tmp/nuke-test2/c.txt
./target/release/nuke -t /tmp/nuke-test2 --force
./target/release/nuke empty --all
```

Expected: confirmation prompt, then all sessions removed. `nuke list` shows "No trash sessions found."

- [ ] **Step 7: Test multi-target**

```bash
mkdir /tmp/t1 /tmp/t2
touch /tmp/t1/x.txt /tmp/t2/y.txt
./target/release/nuke -t /tmp/t1 -t /tmp/t2 --dry-run
```

Expected: preview shows both targets with their counts. Total = 2.

- [ ] **Step 8: Tag release**

```bash
git tag v2.0.0
```

---

## Spec Coverage

| Spec requirement | Task |
|-----------------|------|
| `--dry-run` | Task 3 |
| `--pattern` glob filtering | Task 3 |
| `--exclude` glob filtering | Task 3 |
| `-t` repeatable multi-target | Task 3 |
| `nuke list` session table | Task 4 |
| `nuke restore` interactive picker | Task 4 |
| `nuke empty` multi-select + `--all` | Task 4 |
| Items namespaced by target dir name | Task 2 (`send_to_namespace`) + Task 3 |
| Restore aborts on conflict | Task 2 (`restore_session`) |
| Exclude beats pattern | Task 3 (test + impl) |
| Single confirm for multi-target | Task 3 (`run()`) |
| Dry-run never touches disk | Task 3 (`run()` returns before `trash.create()`) |
| Partial failure: continue + report | Task 3 (error loop in `run()`) |
