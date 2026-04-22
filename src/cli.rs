use clap::Parser;
use std::path::PathBuf;

/// Which items to collect from the target directory.
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    FilesOnly,
    All,
}

/// CLI arguments parsed by clap.
#[derive(Parser, Debug)]
#[command(
    name = "nuke",
    about = "Safely move directory contents to a timestamped trash folder"
)]
pub struct Cli {
    /// Nuke files + subdirectories
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Nuke files only (default behavior)
    #[arg(long = "files-only")]
    pub files_only: bool,

    /// Target directory (default: current directory)
    #[arg(short = 't', long = "target")]
    pub target: Option<PathBuf>,

    /// Skip confirmation prompt
    #[arg(long = "force")]
    pub force: bool,
}

impl Cli {
    /// Resolves the effective scope from the provided flags.
    pub fn scope(&self) -> Scope {
        if self.all {
            Scope::All
        } else {
            Scope::FilesOnly
        }
    }
}
