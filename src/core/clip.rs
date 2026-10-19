use std::{sync::mpsc, time::Duration};

use anyhow::{Context, Result};

/// Copy `value` to the clipboard, wait, then clear it if it is still ours.
///
/// We stay in the foreground for the whole wait: on X11 the clipboard
/// contents vanish when the process that owns them exits.
pub fn copy_and_clear(value: &str, seconds: u64) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("could not open the clipboard")?;
    clipboard
        .set_text(value.to_owned())
        .context("could not copy to the clipboard")?;
    eprintln!("Copied. Clearing the clipboard in {seconds}s (press Ctrl-C to clear it now).");

    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .context("could not install the Ctrl-C handler")?;
    let _ = rx.recv_timeout(Duration::from_secs(seconds));

    if clipboard.get_text().is_ok_and(|current| current == value) {
        clipboard.clear().context("could not clear the clipboard")?;
    }
    Ok(())
}
