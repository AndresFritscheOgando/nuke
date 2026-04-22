use anyhow::{Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Represents the timestamped trash directory for a single nuke operation.
pub struct Trash {
    pub path: PathBuf,
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

    /// Moves a single file or directory into the trash folder.
    pub fn send(&self, item: &Path) -> Result<()> {
        let name = item
            .file_name()
            .with_context(|| format!("item '{}' has no file name", item.display()))?;
        let dest = self.path.join(name);
        move_item(item, &dest)
    }
}

/// Moves `src` to `dest`, falling back to copy+remove on cross-device links.
fn move_item(src: &Path, dest: &Path) -> Result<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        // EXDEV (18): cross-device rename — fall back to copy then remove.
        // Safe because the copy to trash completes before the original is removed.
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_all(src, dest)?;
            remove_all(src)
        }
        Err(e) => Err(e).with_context(|| format!("failed to move '{}' to trash", src.display())),
    }
}

fn copy_all(src: &Path, dest: &Path) -> Result<()> {
    if src.is_file() {
        std::fs::copy(src, dest)
            .with_context(|| format!("failed to copy '{}' to '{}'", src.display(), dest.display()))?;
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
                format!("failed to copy '{}' to '{}'", entry.path().display(), target.display())
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
