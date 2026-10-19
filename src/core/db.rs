use anyhow::{Result, bail};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use zeroize::Zeroizing;

pub struct Secret {
    pub name: String,
    pub value: Zeroizing<String>,
}

fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("the secret name can't be empty");
    }
    Ok(())
}

fn is_constraint(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::SqliteFailure(e, _) if e.code == ErrorCode::ConstraintViolation)
}

pub fn exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM secrets WHERE name = ?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn insert_secret(conn: &Connection, name: &str, value: &str) -> Result<()> {
    check_name(name)?;
    match conn.execute(
        "INSERT INTO secrets (name, value) VALUES (?1, ?2)",
        params![name, value],
    ) {
        Ok(_) => Ok(()),
        Err(err) if is_constraint(&err) => bail!("a secret named `{name}` already exists"),
        Err(err) => Err(err.into()),
    }
}

pub fn update_secret(conn: &Connection, name: &str, value: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE secrets SET value = ?1 WHERE name = ?2",
        params![value, name],
    )?;
    if changed == 0 {
        bail!("no secret named `{name}`");
    }
    Ok(())
}

pub fn get_secret(conn: &Connection, name: &str) -> Result<Zeroizing<String>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM secrets WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .optional()?;
    match value {
        Some(v) => Ok(Zeroizing::new(v)),
        None => bail!("no secret named `{name}`"),
    }
}

pub fn list_names(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM secrets ORDER BY name")?;
    let names = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(names)
}

pub fn get_secrets(conn: &Connection) -> Result<Vec<Secret>> {
    let mut stmt = conn.prepare("SELECT name, value FROM secrets ORDER BY name")?;
    let secrets = stmt
        .query_map([], |r| {
            Ok(Secret {
                name: r.get(0)?,
                value: Zeroizing::new(r.get(1)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(secrets)
}

pub fn delete_secret(conn: &Connection, name: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM secrets WHERE name = ?1", params![name])?;
    if changed == 0 {
        bail!("no secret named `{name}`");
    }
    Ok(())
}

pub fn rename_secret(conn: &Connection, old: &str, new: &str) -> Result<()> {
    check_name(new)?;
    match conn.execute(
        "UPDATE secrets SET name = ?1 WHERE name = ?2",
        params![new, old],
    ) {
        Ok(0) => bail!("no secret named `{old}`"),
        Ok(_) => Ok(()),
        Err(err) if is_constraint(&err) => bail!("a secret named `{new}` already exists"),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE secrets (name TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn insert_get_update_delete() {
        let c = conn();
        insert_secret(&c, "a", "1").unwrap();
        assert_eq!(*get_secret(&c, "a").unwrap(), "1");
        update_secret(&c, "a", "2").unwrap();
        assert_eq!(*get_secret(&c, "a").unwrap(), "2");
        delete_secret(&c, "a").unwrap();
        assert!(get_secret(&c, "a").is_err());
    }

    #[test]
    fn duplicates_and_missing_are_errors() {
        let c = conn();
        insert_secret(&c, "a", "1").unwrap();
        assert!(
            insert_secret(&c, "a", "2")
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );
        assert!(update_secret(&c, "nope", "x").is_err());
        assert!(delete_secret(&c, "nope").is_err());
        assert!(insert_secret(&c, "  ", "x").is_err());
    }

    #[test]
    fn rename_moves_the_value_and_refuses_clashes() {
        let c = conn();
        insert_secret(&c, "a", "1").unwrap();
        insert_secret(&c, "b", "2").unwrap();
        assert!(rename_secret(&c, "a", "b").is_err());
        rename_secret(&c, "a", "c").unwrap();
        assert_eq!(*get_secret(&c, "c").unwrap(), "1");
        assert!(rename_secret(&c, "a", "d").is_err());
    }

    #[test]
    fn names_are_sorted() {
        let c = conn();
        for n in ["b", "c", "a"] {
            insert_secret(&c, n, "x").unwrap();
        }
        assert_eq!(list_names(&c).unwrap(), ["a", "b", "c"]);
    }
}
