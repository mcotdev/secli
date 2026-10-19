# Secli
<div align="center">
<img src="https://user-images.githubusercontent.com/63192115/174482213-e726fe0e-0524-4775-9804-819b8bacb91b.png" alt="logo" height="128" width="128" />
  <br />
  
  <p align="center">
  <img src="https://img.shields.io/crates/v/secli"> <img src="https://img.shields.io/crates/d/secli" /> <img src="https://img.shields.io/github/license/AnishDe12020/secli?color=blue"> <img src="https://img.shields.io/tokei/lines/github/AnishDe12020/secli?color=pink&label=lines%20of%20code"> <img src="https://img.shields.io/github/languages/top/AnishDe12020/secli?color=%230xfffff">
</p>

**A CLI to store secrets locally**
  
  
  </div>
 <br />
 
 <a href="https://www.producthunt.com/posts/secli?utm_source=badge-featured&utm_medium=badge&utm_souce=badge-secli" target="_blank"><img src="https://api.producthunt.com/widgets/embed-image/v1/featured.svg?post_id=349960&theme=dark" alt="Secli - A&#0032;CLI&#0032;to&#0032;store&#0032;secrets&#0032;locally | Product Hunt" style="width: 250px; height: 54px;" width="250" height="54" /></a>

## Why secli?

If you use a plugin that suggests commands (like [`zsh-autosuggestions`](https://github.com/zsh-users/zsh-autosuggestions)), a secret typed on the command line can show up in a suggestion.
Secrets also leak when you record a tutorial or share your screen.
`secli` keeps them out of your shell history and off your screen until you ask for one.

## Is it secure?

Your secrets are stored in an encrypted [SQLCipher](https://www.zetetic.net/sqlcipher/) database (AES-256) in your data directory.
The app is completely offline, and no data is sent across the internet.
It is also open source, so you can read the code.

**Where the key lives.**
On first use, `secli` makes a random 256-bit key and keeps it in your OS keyring (macOS Keychain, Windows Credential Manager, or a Secret Service such as GNOME Keyring or KWallet).
If no keyring is available, it asks you for a passphrase instead.
Run `secli key show` once and keep the key somewhere safe, so a lost keyring does not mean lost secrets.

**What this protects against.**
It protects the file at rest: backups, synced folders, and other users or programs reading your disk.
It also keeps values off your screen: input is hidden, `list` shows names only, and `get --copy` clears the clipboard.

**What it does not protect against.**
Anything running as you while the keyring is unlocked can ask the keyring for the key.
Malware that runs as you can also read the memory of a running `secli`.
`secli get` prints the secret, so it lands in your terminal scrollback.
Clipboard managers can keep a copy of what you copied.
Secrets are cleared from memory when `secli` is done with them, but the libraries it uses make their own short-lived copies.

The data directory and database file are private to your user (`0700` and `0600`).

## Install

```sh-session
$ cargo install secli
```

## Usage

```sh-session
$ secli add github-token
  ? Enter the value for this secret: ********
  ? Confirmation: ********

$ secli get github-token
  ghp_example

$ secli run -e GITHUB_TOKEN=github-token -- gh repo list
```

Leave the name out of `get`, `update`, `delete` or `rename` to pick one from a list.
Pipe a value in to skip the prompt:

```sh-session
$ pass show github | secli add github-token
```

## Commands

| Command | What it does |
|---|---|
| `secli add [NAME]` | Add a secret. Input is hidden. Pipe a value in to skip the prompt. Aliases: `new`, `insert`, `create`. |
| `secli get [NAME]` | Print a secret. `--copy` copies it and clears the clipboard after `--clear-after` seconds (default 30). `--json` prints JSON. Aliases: `read`, `show`. |
| `secli list` | List secret names. `--reveal` shows values too. `--json` prints JSON. Alias: `ls`. |
| `secli update [NAME]` | Replace a value, after confirmation. |
| `secli delete [NAME]` | Delete a secret, after confirmation. Alias: `remove`. |
| `secli rename [OLD] [NEW]` | Rename a secret. |
| `secli run -e [VAR=]NAME -- CMD` | Run a command with secrets in its environment. Repeat `-e` for several. |
| `secli export [-o FILE]` | Write all secrets as JSON, in plain text. A file must not exist yet and gets mode `0600`. |
| `secli import [FILE]` | Read an export. Existing secrets are skipped unless you pass `--overwrite`. |
| `secli key show` | Print the database key so you can back it up. |
| `secli key restore` | Put a backed-up key back into the keyring. |
| `secli completions SHELL` | Print the shell completion setup. |

`update` and `delete` ask before they act.
Pass `--yes` (`-y`) to skip the question, which is required when there is no terminal.

### Scripting

- Errors go to stderr, and failures exit with a non-zero code.
- `secli get NAME` prints only the value, so `$(secli get NAME)` works.
- `secli list` prints one name per line when piped.
- `secli run` passes the child's exit code through, and hides `SECLI_PASSPHRASE` from it.
- Press Esc or Ctrl-C at a prompt to cancel (exit code 130).

### Shell completion

Completion also suggests your secret names.
Add one line to your shell config:

```sh
# bash
source <(secli completions bash)
# zsh
source <(secli completions zsh)
# fish
secli completions fish | source
```

Name completion never prompts.
It works when the key is in the keyring, or when `SECLI_PASSPHRASE` is set.

## Settings

| Variable | Meaning |
|---|---|
| `SECLI_PASSPHRASE` | Use this passphrase instead of the keyring. Handy for scripts. Environment variables are visible to other processes you run, so use it with care. |
| `SECLI_DATA_DIR` | Store the database here instead of the default data directory. |

## Upgrading from 1.x

Version 1 stored secrets in plain text.
The first time you run version 2, `secli` encrypts your existing database in place.
It checks that every secret came through before it replaces the old file.
Back up `secli.db` first if you want a safety net, and delete the backup afterwards because it is not encrypted.
Deleting a file does not wipe its old contents from the disk.

## Development

```sh-session
$ cargo test                  # unit and end-to-end tests
$ cargo test -- --ignored     # also tests the real OS keyring
```

The end-to-end tests use a temporary data directory and a passphrase, so they never touch your real secrets.
