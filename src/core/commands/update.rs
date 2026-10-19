use anyhow::{Result, bail};
use colored::Colorize;
use rusqlite::Connection;

use super::{pick_name, read_value};
use crate::core::{db, prompt};

pub fn update(conn: &Connection, name: Option<String>, yes: bool) -> Result<()> {
    // A piped value uses up stdin, so we can't also ask a question.
    if !yes && !prompt::is_interactive() {
        bail!("this needs confirmation; pass --yes to skip the prompt");
    }
    let name = pick_name(conn, name, "Which secret do you want to update?")?;
    if !db::exists(conn, &name)? {
        bail!("no secret named `{name}`");
    }
    prompt::confirm_or_yes(yes, &format!("Replace the value of `{name}`?"))?;
    let value = read_value("Enter the new value for this secret:")?;

    db::update_secret(conn, &name, &value)?;
    println!("{}", format!("Updated `{}`", name.cyan()).green());
    Ok(())
}
