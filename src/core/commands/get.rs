use anyhow::Result;
use rusqlite::Connection;
use serde_json::json;

use super::pick_name;
use crate::core::{clip, db};

pub fn get(
    conn: &Connection,
    name: Option<String>,
    copy: bool,
    clear_after: u64,
    as_json: bool,
) -> Result<()> {
    let name = pick_name(conn, name, "Which secret do you want?")?;
    let value = db::get_secret(conn, &name)?;

    if copy {
        return clip::copy_and_clear(&value, clear_after);
    }
    if as_json {
        println!("{}", json!({ "name": name, "value": &*value }));
    } else {
        println!("{}", *value);
    }
    Ok(())
}
