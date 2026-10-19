use std::{
    fmt,
    io::{self, IsTerminal, Read},
};

use anyhow::{Result, bail};
use inquire::{
    Confirm, InquireError, Password, PasswordDisplayMode, Select, Text, validator::Validation,
};
use zeroize::Zeroizing;

/// The user pressed Esc or Ctrl-C at a prompt. `main` exits quietly for this.
#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

fn map_err(err: InquireError) -> anyhow::Error {
    match err {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => Cancelled.into(),
        err => err.into(),
    }
}

/// True when we can talk to a person: stdin and stderr are both terminals.
pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

pub fn text(message: &str) -> Result<String> {
    let answer = Text::new(message)
        .with_validator(|s: &str| {
            Ok(if s.trim().is_empty() {
                Validation::Invalid("the name can't be empty".into())
            } else {
                Validation::Valid
            })
        })
        .prompt()
        .map_err(map_err)?;
    Ok(answer)
}

/// Hidden input, asked twice to catch typos.
pub fn secret(message: &str) -> Result<Zeroizing<String>> {
    let answer = Password::new(message)
        .with_display_mode(PasswordDisplayMode::Hidden)
        .prompt()
        .map_err(map_err)?;
    Ok(Zeroizing::new(answer))
}

/// Hidden input, asked once. Used to unlock the database.
pub fn secret_once(message: &str) -> Result<Zeroizing<String>> {
    let answer = Password::new(message)
        .with_display_mode(PasswordDisplayMode::Hidden)
        .without_confirmation()
        .prompt()
        .map_err(map_err)?;
    Ok(Zeroizing::new(answer))
}

pub fn new_passphrase() -> Result<Zeroizing<String>> {
    let answer = Password::new("Choose a passphrase for the secrets database:")
        .with_display_mode(PasswordDisplayMode::Hidden)
        .with_validator(|s: &str| {
            Ok(if s.chars().count() < 8 {
                Validation::Invalid("use at least 8 characters".into())
            } else {
                Validation::Valid
            })
        })
        .prompt()
        .map_err(map_err)?;
    Ok(Zeroizing::new(answer))
}

pub fn confirm(message: &str) -> Result<bool> {
    Confirm::new(message)
        .with_default(false)
        .prompt()
        .map_err(map_err)
}

pub fn select(message: &str, options: Vec<String>) -> Result<String> {
    Select::new(message, options).prompt().map_err(map_err)
}

/// Read a whole value from piped stdin. Strips one trailing newline only.
pub fn read_stdin() -> Result<Zeroizing<String>> {
    let mut buf = Zeroizing::new(String::new());
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn read_stdin_value() -> Result<Zeroizing<String>> {
    let mut value = read_stdin()?;
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    if value.is_empty() {
        bail!("the value read from stdin is empty");
    }
    Ok(value)
}

/// Ask the user to confirm a risky action. Without a terminal, `--yes` is required.
pub fn confirm_or_yes(yes: bool, message: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !is_interactive() {
        bail!("this needs confirmation; pass --yes to skip the prompt");
    }
    if !confirm(message)? {
        return Err(Cancelled.into());
    }
    Ok(())
}
