use std::io::{IsTerminal, stdout};

use anyhow::Result;
use colored::Colorize;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::core::db;

pub fn list(conn: &Connection, reveal: bool, as_json: bool) -> Result<()> {
    let secrets = db::get_secrets(conn)?;

    if as_json {
        let items: Vec<Value> = secrets
            .iter()
            .map(|s| {
                if reveal {
                    json!({ "name": s.name, "value": &*s.value })
                } else {
                    json!({ "name": s.name })
                }
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    let pretty = stdout().is_terminal();
    if secrets.is_empty() {
        if pretty {
            println!(
                "No secrets found. Add one by running {}{}{}",
                "`".cyan(),
                "secli add".green(),
                "`".cyan()
            );
        }
        return Ok(());
    }
    if pretty {
        let noun = if secrets.len() == 1 {
            "secret"
        } else {
            "secrets"
        };
        println!("{}", format!("{} {noun} found", secrets.len()).green());
    }
    for secret in &secrets {
        if reveal {
            println!("{}: {}", secret.name, *secret.value);
        } else {
            println!("{}", secret.name);
        }
    }
    Ok(())
}
