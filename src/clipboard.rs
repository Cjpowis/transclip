//! Copying to the clipboard of whatever device you're looking at.
//!
//! Uses OSC 52: a terminal escape code that asks your terminal app (Termius
//! on the iPhone, for example) to put text on its own clipboard. It travels
//! through SSH untouched, so it reaches the phone, not the Mac. When you're
//! sat at the Mac itself, pbcopy is used as well.

use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use base64::Engine;

pub fn copy(text: &str) -> Result<()> {
    let local = !over_ssh() && cfg!(target_os = "macos");
    if local {
        pbcopy(text)?;
    }
    let osc = osc52(text);
    match OpenOptions::new().write(true).open("/dev/tty") {
        Ok(mut tty) => {
            tty.write_all(osc.as_bytes())?;
            tty.flush()?;
        }
        Err(e) if !local => return Err(e).context("no terminal to copy through"),
        Err(_) => {}
    }
    Ok(())
}

fn osc52(text: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let osc = format!("\x1b]52;c;{b64}\x07");
    if std::env::var_os("TMUX").is_some() {
        // tmux only passes it on when wrapped like this.
        format!("\x1bPtmux;\x1b{osc}\x1b\\")
    } else {
        osc
    }
}

fn over_ssh() -> bool {
    ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT"]
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

fn pbcopy(text: &str) -> Result<()> {
    // Without a UTF-8 locale pbcopy reads its input as plain ASCII and
    // mangles accents, curly quotes and symbols. LC_ALL, because it wins
    // over any LC_CTYPE or LANG the shell passes down.
    let mut child = Command::new("pbcopy")
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .spawn()
        .context("couldn't run pbcopy")?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(text.as_bytes())?;
    if !child.wait()?.success() {
        anyhow::bail!("pbcopy failed");
    }
    Ok(())
}
