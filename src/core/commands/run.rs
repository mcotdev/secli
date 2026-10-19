use std::process::Command;

use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::core::{db, vault::PASSPHRASE_ENV};

fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Run a command with secrets in its environment. Each spec is `NAME` or `VAR=NAME`.
pub fn run(conn: &Connection, specs: &[String], command: &[String]) -> Result<()> {
    let mut vars = Vec::new();
    for spec in specs {
        let (var, name) = spec.split_once('=').unwrap_or((spec, spec));
        if !is_env_name(var) {
            bail!("`{var}` is not a valid environment variable name; use VAR=NAME to choose one");
        }
        vars.push((var, db::get_secret(conn, name)?));
    }

    let (program, args) = command.split_first().expect("clap requires a command");
    let mut cmd = Command::new(program);
    cmd.args(args).env_remove(PASSPHRASE_ENV);
    for (var, value) in &vars {
        cmd.env(var, &***value);
    }

    #[cfg(unix)]
    {
        // exec replaces this process, so the exit code and signals pass straight through.
        let err = std::os::unix::process::CommandExt::exec(&mut cmd);
        bail!("could not run `{program}`: {err}");
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(test)]
mod tests {
    use super::is_env_name;

    #[test]
    fn env_names() {
        assert!(is_env_name("API_KEY"));
        assert!(is_env_name("_x1"));
        assert!(!is_env_name("1x"));
        assert!(!is_env_name("my-key"));
        assert!(!is_env_name(""));
    }
}
