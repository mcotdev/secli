use anyhow::Result;
use colored::Colorize;
use rusqlite::Connection;

use super::pick_name;
use crate::core::{db, prompt};

pub fn delete(conn: &Connection, name: Option<String>, yes: bool) -> Result<()> {
    let name = pick_name(conn, name, "Which secret do you want to delete?")?;
    if !db::exists(conn, &name)? {
        anyhow::bail!("no secret named `{name}`");
    }
    prompt::confirm_or_yes(yes, &format!("Delete `{name}`? This can't be undone."))?;

    db::delete_secret(conn, &name)?;
    println!("{}", format!("Deleted `{}`", name.cyan()).green());
    Ok(())
}
