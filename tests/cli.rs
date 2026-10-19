//! End-to-end tests. Every test uses its own temporary data directory and a
//! passphrase from the environment, so none of them touch the real database
//! or the OS keyring.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

const PASS: &str = "correct horse battery";

fn secli(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("secli").unwrap();
    cmd.env("SECLI_DATA_DIR", dir.path())
        .env("SECLI_PASSPHRASE", PASS)
        .env("NO_COLOR", "1")
        .env_remove("COMPLETE");
    cmd
}

fn add(dir: &TempDir, name: &str, value: &str) {
    secli(dir)
        .args(["add", name])
        .write_stdin(value)
        .assert()
        .success();
}

#[test]
fn add_get_list_round_trip() {
    let dir = TempDir::new().unwrap();
    add(&dir, "api", "s3cret\n");

    secli(&dir)
        .args(["get", "api"])
        .assert()
        .success()
        .stdout("s3cret\n");
    // Only one trailing newline is stripped.
    add(&dir, "multi", "a\nb\n\n");
    secli(&dir)
        .args(["get", "multi"])
        .assert()
        .success()
        .stdout("a\nb\n\n");
}

#[test]
fn list_hides_values_unless_revealed() {
    let dir = TempDir::new().unwrap();
    add(&dir, "b", "two");
    add(&dir, "a", "one");

    secli(&dir).arg("list").assert().success().stdout("a\nb\n");
    secli(&dir)
        .args(["ls", "--reveal"])
        .assert()
        .success()
        .stdout("a: one\nb: two\n");
    secli(&dir)
        .args(["list", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"a\""))
        .stdout(predicate::str::contains("one").not());
    secli(&dir)
        .args(["list", "--json", "--reveal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\": \"one\""));
}

#[test]
fn errors_go_to_stderr_with_a_failing_exit_code() {
    let dir = TempDir::new().unwrap();
    add(&dir, "a", "1");

    secli(&dir)
        .args(["get", "missing"])
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("no secret named `missing`"));
    secli(&dir)
        .args(["add", "a"])
        .write_stdin("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
    // The failed add did not change the value.
    secli(&dir).args(["get", "a"]).assert().stdout("1\n");
}

#[test]
fn empty_piped_value_and_missing_name_are_rejected() {
    let dir = TempDir::new().unwrap();
    secli(&dir)
        .args(["add", "a"])
        .write_stdin("")
        .assert()
        .failure();
    secli(&dir).arg("add").write_stdin("x").assert().failure();
    secli(&dir).arg("get").assert().failure();
}

#[test]
fn update_and_delete_need_yes_without_a_terminal() {
    let dir = TempDir::new().unwrap();
    add(&dir, "a", "1");

    secli(&dir)
        .args(["update", "a"])
        .write_stdin("2")
        .assert()
        .failure();
    secli(&dir).args(["delete", "a"]).assert().failure();
    secli(&dir).args(["get", "a"]).assert().stdout("1\n");

    secli(&dir)
        .args(["update", "a", "--yes"])
        .write_stdin("2")
        .assert()
        .success();
    secli(&dir).args(["get", "a"]).assert().stdout("2\n");
    secli(&dir).args(["delete", "a", "-y"]).assert().success();
    secli(&dir).args(["get", "a"]).assert().failure();
    secli(&dir).args(["delete", "a", "-y"]).assert().failure();
}

#[test]
fn rename_moves_a_secret() {
    let dir = TempDir::new().unwrap();
    add(&dir, "old", "v");
    add(&dir, "taken", "t");

    secli(&dir)
        .args(["rename", "old", "taken"])
        .assert()
        .failure();
    secli(&dir)
        .args(["rename", "old", "new"])
        .assert()
        .success();
    secli(&dir).args(["get", "new"]).assert().stdout("v\n");
    secli(&dir).args(["get", "old"]).assert().failure();
}

#[test]
fn export_and_import_move_secrets_between_databases() {
    let (src, dst) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    add(&src, "a", "1");
    add(&src, "b", "2");
    let out = src.path().join("out.json");

    secli(&src)
        .args(["export", "-o"])
        .arg(&out)
        .assert()
        .success();
    // Refuses to overwrite an existing file.
    secli(&src)
        .args(["export", "-o"])
        .arg(&out)
        .assert()
        .failure();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    add(&dst, "a", "keep");
    secli(&dst).arg("import").arg(&out).assert().success();
    secli(&dst).args(["get", "a"]).assert().stdout("keep\n");
    secli(&dst).args(["get", "b"]).assert().stdout("2\n");

    secli(&dst)
        .arg("import")
        .arg(&out)
        .arg("--overwrite")
        .assert()
        .success();
    secli(&dst).args(["get", "a"]).assert().stdout("1\n");

    // Stdin works too, and junk is rejected.
    secli(&dst)
        .args(["import", "-"])
        .write_stdin(fs::read(&out).unwrap())
        .assert()
        .success();
    secli(&dst)
        .arg("import")
        .write_stdin("not json")
        .assert()
        .failure();
}

#[test]
fn run_passes_secrets_and_hides_the_passphrase() {
    let dir = TempDir::new().unwrap();
    add(&dir, "token", "abc123");

    secli(&dir)
        .args([
            "run",
            "-e",
            "API_TOKEN=token",
            "--",
            "sh",
            "-c",
            "printf '%s' \"$API_TOKEN\"",
        ])
        .assert()
        .success()
        .stdout("abc123");
    secli(&dir)
        .args([
            "run",
            "-e",
            "token",
            "--",
            "sh",
            "-c",
            "printf '%s' \"$token\"",
        ])
        .assert()
        .success()
        .stdout("abc123");
    secli(&dir)
        .args([
            "run",
            "-e",
            "token",
            "--",
            "sh",
            "-c",
            "printf '%s' \"${SECLI_PASSPHRASE-unset}\"",
        ])
        .assert()
        .stdout("unset");
    // The child's exit code comes through.
    secli(&dir)
        .args(["run", "-e", "token", "--", "sh", "-c", "exit 7"])
        .assert()
        .code(7);
    // Bad input fails before anything runs.
    secli(&dir)
        .args(["run", "-e", "nope", "--", "true"])
        .assert()
        .failure();
    secli(&dir)
        .args(["run", "-e", "my-var=token", "--", "true"])
        .assert()
        .failure();
}

#[test]
fn database_is_encrypted_and_private() {
    let dir = TempDir::new().unwrap();
    add(&dir, "visible-name", "plain-text-needle");

    let bytes = fs::read(dir.path().join("secli.db")).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    for needle in [&b"plain-text-needle"[..], b"visible-name"] {
        assert!(!bytes.windows(needle.len()).any(|w| w == needle));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &std::path::Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.path()), 0o700);
        assert_eq!(mode(&dir.path().join("secli.db")), 0o600);
    }
}

#[test]
fn wrong_passphrase_is_refused() {
    let dir = TempDir::new().unwrap();
    add(&dir, "a", "1");

    secli(&dir)
        .env("SECLI_PASSPHRASE", "not the passphrase")
        .args(["get", "a"])
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("could not unlock the database"));
    // Without a terminal and without a passphrase there is nothing to ask.
    secli(&dir)
        .env_remove("SECLI_PASSPHRASE")
        .args(["get", "a"])
        .assert()
        .failure();
}

#[test]
fn old_plaintext_databases_are_migrated() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("secli.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "CREATE TABLE secrets (name TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO secrets VALUES ('old1', 'v1'), ('old2', 'v2')",
            [],
        )
        .unwrap();
    }
    assert!(fs::read(&path).unwrap().starts_with(b"SQLite format 3"));

    secli(&dir)
        .args(["list", "--reveal"])
        .assert()
        .success()
        .stdout("old1: v1\nold2: v2\n")
        .stderr(predicate::str::contains(
            "Encrypted your existing secrets database (2 secrets)",
        ));

    let bytes = fs::read(&path).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    assert!(!bytes.windows(2).any(|w| w == b"v1"));
    assert!(!dir.path().join("secli.db.migrating").exists());
    // And it keeps working afterwards.
    secli(&dir).args(["get", "old2"]).assert().stdout("v2\n");
}

#[test]
fn key_show_has_nothing_to_show_for_passphrase_databases() {
    let dir = TempDir::new().unwrap();
    add(&dir, "a", "1");
    secli(&dir)
        .args(["key", "show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("uses a passphrase"));
}

#[test]
fn completion_script_is_available() {
    let dir = TempDir::new().unwrap();
    secli(&dir)
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("secli"));
}

/// Needs a real OS keyring, so it only runs when asked:
/// `cargo test -- --ignored`. It uses its own keyring entry and removes it.
#[test]
#[ignore = "uses the real OS keyring"]
fn keyring_key_creation_backup_and_restore() {
    const SERVICE: &str = "secli-test";
    let dir = TempDir::new().unwrap();
    let cmd = || {
        let mut c = Command::cargo_bin("secli").unwrap();
        c.env("SECLI_DATA_DIR", dir.path())
            .env("SECLI_KEYRING_SERVICE", SERVICE)
            .env_remove("SECLI_PASSPHRASE")
            .env_remove("COMPLETE");
        c
    };
    let cleanup = || {
        let _ = keyring::Entry::new(SERVICE, "database-key").and_then(|e| e.delete_credential());
    };
    cleanup();

    cmd().args(["add", "k"]).write_stdin("v").assert().success();
    cmd().args(["get", "k"]).assert().stdout("v\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("key-source")).unwrap(),
        "keyring"
    );

    let key = cmd().args(["key", "show"]).output().unwrap().stdout;
    let key = String::from_utf8(key).unwrap().trim().to_string();
    assert_eq!(key.len(), 64);

    // Lose the key: the database can't be opened, and we are told how to recover.
    cleanup();
    cmd()
        .args(["get", "k"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("secli key restore"));
    // A wrong key is refused; the right one restores access.
    cmd()
        .args(["key", "restore"])
        .write_stdin("ab".repeat(32))
        .assert()
        .failure();
    cmd()
        .args(["key", "restore"])
        .write_stdin(key)
        .assert()
        .success();
    cmd().args(["get", "k"]).assert().stdout("v\n");

    cleanup();
}
