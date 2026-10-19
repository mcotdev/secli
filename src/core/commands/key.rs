use std::io::IsTerminal;

use anyhow::Result;
use colored::Colorize;

use crate::core::{prompt, vault};

pub fn show() -> Result<()> {
    let key = vault::show_key()?;
    eprintln!("Warning: this key opens your whole database. Store it somewhere safe.");
    println!("{}", *key);
    Ok(())
}

pub fn restore() -> Result<()> {
    let key = if std::io::stdin().is_terminal() {
        prompt::secret_once("Paste the database key:")?
    } else {
        prompt::read_stdin_value()?
    };
    vault::restore_key(&key)?;
    println!("{}", "Key restored to the OS keyring".green());
    Ok(())
}
