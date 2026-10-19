//! Opens the encrypted secrets database and manages its key.
//!
//! The database is a SQLCipher file. Its key comes from one of two places,
//! recorded in a small plain-text file next to it (`key-source`):
//!
//! - `keyring`: a random 256-bit key kept in the OS keyring.
//! - `passphrase`: a passphrase typed by the user, or set in `SECLI_PASSPHRASE`.
//!
//! The two are different keys, so we never fall back from one to the other
//! for an existing database. We only create a key when there is no database yet.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use rusqlite::{Connection, params};
use zeroize::Zeroizing;

use super::prompt;

pub const PASSPHRASE_ENV: &str = "SECLI_PASSPHRASE";
const DATA_DIR_ENV: &str = "SECLI_DATA_DIR";
const KEYRING_SERVICE_ENV: &str = "SECLI_KEYRING_SERVICE";
const KEYRING_USER: &str = "database-key";
const PLAINTEXT_HEADER: &[u8; 16] = b"SQLite format 3\0";

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS secrets (
    name TEXT PRIMARY KEY,
    value TEXT
)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Keyring,
    Passphrase,
}

impl KeySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::Passphrase => "passphrase",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "keyring" => Some(Self::Keyring),
            "passphrase" => Some(Self::Passphrase),
            _ => None,
        }
    }
}

/// The text we hand to `PRAGMA key`: a raw hex key (`x'..'`) or a passphrase.
pub struct Key(Zeroizing<String>);

impl Key {
    fn raw(hex: &str) -> Self {
        Self(Zeroizing::new(format!("x'{hex}'")))
    }

    fn passphrase(text: Zeroizing<String>) -> Self {
        Self(text)
    }
}

// ---- paths and permissions ----

pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let base = dirs::data_dir().context("could not find the user data directory")?;
    Ok(base.join("secli"))
}

fn db_path(dir: &Path) -> PathBuf {
    dir.join("secli.db")
}

fn source_path(dir: &Path) -> PathBuf {
    dir.join("key-source")
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("could not set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// Create the data directory and make it private, even if it already existed.
fn ensure_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .with_context(|| format!("could not create data directory {}", dir.display()))?;
    restrict(dir, 0o700)
}

fn read_source(dir: &Path) -> Option<KeySource> {
    fs::read_to_string(source_path(dir))
        .ok()
        .and_then(|text| KeySource::parse(&text))
}

fn write_source(dir: &Path, source: KeySource) -> Result<()> {
    let path = source_path(dir);
    fs::write(&path, source.as_str())
        .with_context(|| format!("could not write {}", path.display()))?;
    restrict(&path, 0o600)
}

// ---- key sources ----

fn env_passphrase() -> Option<Zeroizing<String>> {
    std::env::var(PASSPHRASE_ENV)
        .ok()
        .filter(|p| !p.is_empty())
        .map(Zeroizing::new)
}

fn keyring_entry() -> Result<keyring::Entry> {
    let service = std::env::var(KEYRING_SERVICE_ENV).unwrap_or_else(|_| "secli".into());
    keyring::Entry::new(&service, KEYRING_USER).map_err(|e| anyhow!("OS keyring unavailable: {e}"))
}

fn keyring_get() -> Result<Option<Zeroizing<String>>> {
    match keyring_entry()?.get_password() {
        Ok(hex) => Ok(Some(Zeroizing::new(hex))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("could not read the OS keyring: {e}")),
    }
}

fn random_hex_key() -> Result<Zeroizing<String>> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *bytes).map_err(|e| anyhow!("could not get random bytes: {e}"))?;
    let mut hex = Zeroizing::new(String::with_capacity(64));
    for b in bytes.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    Ok(hex)
}

fn keyring_get_or_create() -> Result<Zeroizing<String>> {
    if let Some(hex) = keyring_get()? {
        return Ok(hex);
    }
    let hex = random_hex_key()?;
    keyring_entry()?
        .set_password(&hex)
        .map_err(|e| anyhow!("could not save the key in the OS keyring: {e}"))?;
    Ok(hex)
}

fn is_hex_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Pick the key for a database that does not exist yet.
fn new_key(interactive: bool) -> Result<(KeySource, Key)> {
    if let Some(p) = env_passphrase() {
        return Ok((KeySource::Passphrase, Key::passphrase(p)));
    }
    match keyring_get_or_create() {
        Ok(hex) if is_hex_key(&hex) => Ok((KeySource::Keyring, Key::raw(&hex))),
        Ok(_) => {
            bail!("the key in the OS keyring is not valid; remove the `secli` entry and retry")
        }
        Err(err) => {
            if !interactive {
                bail!("{err}. Set {PASSPHRASE_ENV} to use a passphrase instead");
            }
            eprintln!("{err}. Using a passphrase instead.");
            let p = prompt::new_passphrase()?;
            Ok((KeySource::Passphrase, Key::passphrase(p)))
        }
    }
}

/// Find the key for a database that already exists. Never creates a key.
fn existing_key(dir: &Path, interactive: bool) -> Result<Key> {
    let source = read_source(dir).unwrap_or(if env_passphrase().is_some() {
        KeySource::Passphrase
    } else {
        KeySource::Keyring
    });

    match source {
        KeySource::Keyring => match keyring_get()? {
            Some(hex) if is_hex_key(&hex) => Ok(Key::raw(&hex)),
            Some(_) => bail!("the key in the OS keyring is not valid"),
            None => bail!(
                "the database key is not in the OS keyring. \
                 If you saved a copy with `secli key show`, put it back with `secli key restore`"
            ),
        },
        KeySource::Passphrase => {
            if let Some(p) = env_passphrase() {
                return Ok(Key::passphrase(p));
            }
            if !interactive {
                bail!("the database needs a passphrase; set {PASSPHRASE_ENV}");
            }
            Ok(Key::passphrase(prompt::secret_once(
                "Passphrase for the secrets database:",
            )?))
        }
    }
}

// ---- opening, creating, migrating ----

fn open_with_key(path: &Path, key: &Key) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("could not open database {}", path.display()))?;
    // SQLCipher prints its own errors to stderr on a wrong key; we report ours instead.
    conn.pragma_update(None, "cipher_log_level", "NONE")?;
    conn.pragma_update(None, "key", key.0.as_str())?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| {
        anyhow!("could not unlock the database: wrong key or passphrase, or the file is damaged")
    })?;
    Ok(conn)
}

fn is_plaintext(path: &Path) -> Result<bool> {
    let mut header = [0u8; 16];
    let mut file = File::open(path)?;
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == PLAINTEXT_HEADER),
        Err(_) => Ok(false),
    }
}

fn create(dir: &Path, interactive: bool) -> Result<Connection> {
    let path = db_path(dir);
    let (source, key) = new_key(interactive)?;
    write_source(dir, source)?;
    let conn = open_with_key(&path, &key)?;
    conn.execute(CREATE_TABLE, [])?;
    restrict(&path, 0o600)?;
    Ok(conn)
}

/// Turn a plaintext database from an older secli into an encrypted one.
/// The original is only replaced after the copy is verified.
fn migrate(dir: &Path, interactive: bool) -> Result<Connection> {
    let path = db_path(dir);
    let tmp = dir.join("secli.db.migrating");
    let _ = fs::remove_file(&tmp);

    let (source, key) = new_key(interactive)?;
    let tmp_str = tmp
        .to_str()
        .context("the data directory path is not valid UTF-8")?;

    let before: i64 = {
        let old = Connection::open(&path)?;
        let before = old
            .query_row("SELECT count(*) FROM secrets", [], |r| r.get(0))
            .unwrap_or(0);
        old.execute(
            "ATTACH DATABASE ?1 AS enc KEY ?2",
            params![tmp_str, key.0.as_str()],
        )?;
        old.prepare("SELECT sqlcipher_export('enc')")?
            .query([])?
            .next()?;
        old.execute("DETACH DATABASE enc", [])?;
        before
    };

    let verified = (|| -> Result<()> {
        let new = open_with_key(&tmp, &key)?;
        new.execute(CREATE_TABLE, [])?;
        let after: i64 = new.query_row("SELECT count(*) FROM secrets", [], |r| r.get(0))?;
        if after != before {
            bail!("expected {before} secrets after encrypting, found {after}");
        }
        Ok(())
    })();
    if let Err(err) = verified {
        let _ = fs::remove_file(&tmp);
        return Err(err.context("could not encrypt the existing database; it was left unchanged"));
    }

    restrict(&tmp, 0o600)?;
    write_source(dir, source)?;
    fs::rename(&tmp, &path).context("could not replace the old database")?;
    eprintln!("Encrypted your existing secrets database ({before} secrets).");
    open_with_key(&path, &key)
}

fn open_impl(quiet: bool) -> Result<Connection> {
    let dir = data_dir()?;
    let path = db_path(&dir);
    let interactive = !quiet && prompt::is_interactive();
    let exists = path.metadata().map(|m| m.len() > 0).unwrap_or(false);

    if quiet && (!exists || is_plaintext(&path)?) {
        bail!("no encrypted database to read");
    }
    if !quiet {
        ensure_dir(&dir)?;
    }

    if !exists {
        return create(&dir, interactive);
    }
    if is_plaintext(&path)? {
        return migrate(&dir, interactive);
    }
    let key = existing_key(&dir, interactive)?;
    let conn = open_with_key(&path, &key)?;
    if !quiet {
        restrict(&path, 0o600)?;
    }
    Ok(conn)
}

/// Open (creating or migrating if needed) the secrets database. May prompt.
pub fn open() -> Result<Connection> {
    open_impl(false)
}

/// Open an existing database without prompting, creating or migrating anything.
/// Used by shell completion, which must stay silent and read-only.
pub fn open_quiet() -> Result<Connection> {
    open_impl(true)
}

// ---- key backup and recovery ----

/// The raw database key as hex, for `secli key show`.
pub fn show_key() -> Result<Zeroizing<String>> {
    let dir = data_dir()?;
    if read_source(&dir) == Some(KeySource::Passphrase) {
        bail!("this database uses a passphrase; there is no stored key to show");
    }
    keyring_get()?.ok_or_else(|| anyhow!("no database key found in the OS keyring"))
}

/// Put a saved key back into the OS keyring, after checking it opens the database.
pub fn restore_key(hex: &str) -> Result<()> {
    let hex = hex.trim().to_ascii_lowercase();
    if !is_hex_key(&hex) {
        bail!("the key must be 64 hexadecimal characters");
    }
    let dir = data_dir()?;
    open_with_key(&db_path(&dir), &Key::raw(&hex))
        .context("that key does not open the database")?;
    keyring_entry()?
        .set_password(&hex)
        .map_err(|e| anyhow!("could not save the key in the OS keyring: {e}"))?;
    write_source(&dir, KeySource::Keyring)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_key() -> Key {
        Key::raw(&"ab".repeat(32))
    }

    #[test]
    fn key_source_round_trips() {
        assert_eq!(KeySource::parse("keyring\n"), Some(KeySource::Keyring));
        assert_eq!(KeySource::parse("passphrase"), Some(KeySource::Passphrase));
        assert_eq!(KeySource::parse("other"), None);
    }

    #[test]
    fn hex_key_validation() {
        assert!(is_hex_key(&"0f".repeat(32)));
        assert!(!is_hex_key("abc"));
        assert!(!is_hex_key(&"zz".repeat(32)));
    }

    #[test]
    fn random_keys_are_valid_and_differ() {
        let (a, b) = (random_hex_key().unwrap(), random_hex_key().unwrap());
        assert!(is_hex_key(&a));
        assert_ne!(*a, *b);
    }

    #[test]
    fn encrypted_file_is_not_plaintext_and_needs_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = db_path(dir.path());
        {
            let conn = open_with_key(&path, &raw_key()).unwrap();
            conn.execute(CREATE_TABLE, []).unwrap();
            conn.execute("INSERT INTO secrets VALUES ('a', 'topsecretvalue')", [])
                .unwrap();
        }
        assert!(!is_plaintext(&path).unwrap());
        assert!(
            !fs::read(&path)
                .unwrap()
                .windows(14)
                .any(|w| w == b"topsecretvalue")
        );
        assert!(open_with_key(&path, &Key::raw(&"cd".repeat(32))).is_err());
        assert!(open_with_key(&path, &raw_key()).is_ok());
    }

    #[test]
    fn plaintext_files_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let path = db_path(dir.path());
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE secrets (name TEXT PRIMARY KEY, value TEXT)")
            .unwrap();
        assert!(is_plaintext(&path).unwrap());
    }
}
