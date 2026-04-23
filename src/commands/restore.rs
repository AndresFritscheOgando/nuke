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
