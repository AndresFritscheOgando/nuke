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
