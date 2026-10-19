use anyhow::{Result, bail};
use colored::Colorize;
use rusqlite::Connection;

use super::pick_name;
use crate::core::{db, prompt};

pub fn rename(conn: &Connection, old: Option<String>, new: Option<String>) -> Result<()> {
    let old = pick_name(conn, old, "Which secret do you want to rename?")?;
    let new = match new {
        Some(new) => new,
        None if prompt::is_interactive() => prompt::text("New name:")?,
        None => bail!("the new name is required"),
    };

    db::rename_secret(conn, &old, &new)?;
    println!(
        "{}",
        format!("Renamed `{}` to `{}`", old.cyan(), new.cyan()).green()
    );
    Ok(())
}
