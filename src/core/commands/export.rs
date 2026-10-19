use std::{fs::OpenOptions, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::core::db;

#[derive(Serialize, Deserialize)]
pub struct ExportFile {
    pub version: u32,
    pub secrets: Vec<ExportSecret>,
}

#[derive(Serialize, Deserialize)]
pub struct ExportSecret {
    pub name: String,
    pub value: Zeroizing<String>,
}

pub fn export(conn: &Connection, output: Option<PathBuf>) -> Result<()> {
    let file = ExportFile {
        version: 1,
        secrets: db::get_secrets(conn)?
            .into_iter()
            .map(|s| ExportSecret {
                name: s.name,
                value: s.value,
            })
            .collect(),
    };
    let json = Zeroizing::new(serde_json::to_string_pretty(&file)?);

    eprintln!("Warning: the export holds your secrets in plain text. Keep it safe.");
    match output {
        None => println!("{}", *json),
        Some(path) => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut out = options.open(&path).with_context(|| {
                format!("could not create {} (it must not exist)", path.display())
            })?;
            out.write_all(json.as_bytes())?;
            eprintln!("Wrote {} secrets to {}", file.secrets.len(), path.display());
        }
    }
    Ok(())
}
