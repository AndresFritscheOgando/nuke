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
