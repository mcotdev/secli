use std::{ffi::OsStr, path::PathBuf, process::Command as Process};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clap_complete::{
    Shell,
    engine::{ArgValueCompleter, CompletionCandidate},
};

use crate::core::{
    commands::{add, delete, export, get, import, key, list, rename, run, update},
    db, vault,
};

#[derive(Parser)]
#[command(version, author, about, propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Complete secret names from the database. Never prompts or changes anything.
fn complete_names(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return Vec::new();
    };
    let Ok(conn) = vault::open_quiet() else {
        return Vec::new();
    };
    db::list_names(&conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

#[derive(Subcommand)]
enum Command {
    /// Add a secret (pipe the value in to skip the prompt)
    #[command(visible_aliases = ["new", "insert", "create"])]
    Add {
        /// Name of the secret
        name: Option<String>,
    },
    /// Print a secret, or copy it with --copy
    #[command(visible_aliases = ["read", "show"])]
    Get {
        /// Name of the secret (pick from a list if left out)
        #[arg(add = ArgValueCompleter::new(complete_names))]
        name: Option<String>,
        /// Copy to the clipboard instead of printing, then clear it
        #[arg(short, long)]
        copy: bool,
        /// Seconds to wait before clearing the clipboard
        #[arg(long, value_name = "SECONDS", default_value_t = 30, requires = "copy")]
        clear_after: u64,
        /// Print as JSON
        #[arg(long, conflicts_with = "copy")]
        json: bool,
    },
    /// List secret names (values are hidden unless you pass --reveal)
    #[command(visible_alias = "ls")]
    List {
        /// Show the values too
        #[arg(long)]
        reveal: bool,
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
    /// Replace the value of a secret
    Update {
        /// Name of the secret (pick from a list if left out)
        #[arg(add = ArgValueCompleter::new(complete_names))]
        name: Option<String>,
        /// Don't ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Delete a secret
    #[command(visible_alias = "remove")]
    Delete {
        /// Name of the secret (pick from a list if left out)
        #[arg(add = ArgValueCompleter::new(complete_names))]
        name: Option<String>,
        /// Don't ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Rename a secret
    Rename {
        /// Current name (pick from a list if left out)
        #[arg(add = ArgValueCompleter::new(complete_names))]
        old: Option<String>,
        /// New name
        new: Option<String>,
    },
    /// Run a command with secrets set as environment variables
    Run {
        /// A secret to pass in, as NAME or VAR=NAME (repeat for several)
        #[arg(short, long = "env", value_name = "[VAR=]NAME", required = true)]
        env: Vec<String>,
        /// The command to run, after `--`
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Write all secrets to a JSON file (plain text!)
    Export {
        /// Write to this new file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Read secrets from a JSON export
    Import {
        /// The export file (reads stdin if left out or `-`)
        file: Option<PathBuf>,
        /// Replace secrets that already exist
        #[arg(long)]
        overwrite: bool,
    },
    /// Back up or restore the database key
    #[command(subcommand)]
    Key(KeyCommand),
    /// Print the shell completion setup for your shell
    Completions { shell: Shell },
}

#[derive(Subcommand)]
enum KeyCommand {
    /// Print the database key, so you can keep a backup
    Show,
    /// Put a backed-up key back into the OS keyring
    Restore,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Key(KeyCommand::Show) => key::show(),
            Command::Key(KeyCommand::Restore) => key::restore(),
            Command::Completions { shell } => completions(shell),
            Command::Add { name } => add::add(&vault::open()?, name),
            Command::Get {
                name,
                copy,
                clear_after,
                json,
            } => get::get(&vault::open()?, name, copy, clear_after, json),
            Command::List { reveal, json } => list::list(&vault::open()?, reveal, json),
            Command::Update { name, yes } => update::update(&vault::open()?, name, yes),
            Command::Delete { name, yes } => delete::delete(&vault::open()?, name, yes),
            Command::Rename { old, new } => rename::rename(&vault::open()?, old, new),
            Command::Run { env, command } => run::run(&vault::open()?, &env, &command),
            Command::Export { output } => export::export(&vault::open()?, output),
            Command::Import { file, overwrite } => {
                import::import(&mut vault::open()?, file, overwrite)
            }
        }
    }
}

/// The completion script comes from clap_complete's env-driven mode.
fn completions(shell: Shell) -> Result<()> {
    let exe = std::env::current_exe().context("could not find the secli executable")?;
    let status = Process::new(exe)
        .env("COMPLETE", shell.to_string())
        .status()?;
    std::process::exit(status.code().unwrap_or(1));
}
