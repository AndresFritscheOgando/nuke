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

    /// Moves a single file or directory into the trash folder (legacy flat layout).
    pub fn send(&self, item: &Path) -> Result<()> {
        let name = item
            .file_name()
            .with_context(|| format!("item '{}' has no file name", item.display()))?;
        let dest = self.path.join(name);
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
