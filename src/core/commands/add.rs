use anyhow::{Result, bail};
use colored::Colorize;
use rusqlite::Connection;

use super::read_value;
use crate::core::{db, prompt};

pub fn add(conn: &Connection, name: Option<String>) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None if prompt::is_interactive() => prompt::text("Enter the name/key for this secret:")?,
        None => bail!("a secret name is required"),
    };
    if db::exists(conn, &name)? {
        bail!("a secret named `{name}` already exists; use `secli update` to change it");
    }
    let value = read_value("Enter the value for this secret:")?;

    db::insert_secret(conn, &name, &value)?;
    println!("{}", format!("Added `{}`", name.cyan()).green());
    Ok(())
}
