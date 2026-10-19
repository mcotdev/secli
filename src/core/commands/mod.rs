pub mod add;
pub mod delete;
pub mod export;
pub mod get;
pub mod import;
pub mod key;
pub mod list;
pub mod rename;
pub mod run;
pub mod update;

use anyhow::{Result, bail};
use rusqlite::Connection;

use super::{db, prompt};

/// The name given on the command line, or one picked from the list.
pub(crate) fn pick_name(conn: &Connection, name: Option<String>, message: &str) -> Result<String> {
    if let Some(name) = name {
        return Ok(name);
    }
    if !prompt::is_interactive() {
        bail!("a secret name is required");
    }
    let names = db::list_names(conn)?;
    if names.is_empty() {
        bail!("no secrets yet; add one with `secli add`");
    }
    prompt::select(message, names)
}

/// A new value: piped on stdin if stdin is not a terminal, otherwise asked for.
pub(crate) fn read_value(message: &str) -> Result<zeroize::Zeroizing<String>> {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        prompt::secret(message)
    } else {
        prompt::read_stdin_value()
    }
}
