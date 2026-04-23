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
