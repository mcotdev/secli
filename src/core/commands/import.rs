use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use colored::Colorize;
use rusqlite::Connection;
use zeroize::Zeroizing;

use super::export::ExportFile;
use crate::core::{db, prompt};

pub fn import(conn: &mut Connection, file: Option<PathBuf>, overwrite: bool) -> Result<()> {
    let text = match file {
        Some(path) if path.as_os_str() != "-" => Zeroizing::new(
            fs::read_to_string(&path)
                .with_context(|| format!("could not read {}", path.display()))?,
        ),
        _ => prompt::read_stdin()?,
    };
    let data: ExportFile =
        serde_json::from_str(&text).context("this is not a secli export file")?;
    if data.version != 1 {
        bail!("unsupported export version {}", data.version);
    }

    let tx = conn.transaction()?;
    let (mut added, mut updated, mut skipped) = (0, 0, 0);
    for secret in &data.secrets {
        if db::exists(&tx, &secret.name)? {
            if overwrite {
                db::update_secret(&tx, &secret.name, &secret.value)?;
                updated += 1;
            } else {
                skipped += 1;
            }
        } else {
            db::insert_secret(&tx, &secret.name, &secret.value)?;
            added += 1;
        }
    }
    tx.commit()?;

    println!(
        "{}",
        format!("Imported: {added} added, {updated} updated, {skipped} skipped").green()
    );
    if skipped > 0 {
        eprintln!("Skipped secrets already exist; use --overwrite to replace them.");
    }
    Ok(())
}
