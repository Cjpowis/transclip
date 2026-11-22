//! Saving the transcript as a text file: `--save`, or `s` in the viewer.
//!
//! The folder is checked (and made, if it's missing) before anything is
//! downloaded, so a mistake shows up straight away rather than after the
//! transcribing is done.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::progress::{MUTED, RESET};

/// Downloads, the usual place to look for a new file.
pub fn default_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The folder to save in, ready to use: `~` expanded, made if it's missing
/// (and said so), and checked that a file can be written there.
pub fn prepare(folder: Option<&str>) -> Result<PathBuf> {
    let dir = match folder {
        Some(f) if !f.trim().is_empty() => expand_home(f.trim()),
        _ => default_dir(),
    };
    ready(dir, true)
}

/// Make sure a file can be saved in `dir`, making the folder if it's
/// missing. `announce`: say when a folder is made (not from the viewer,
/// where printing would spoil the screen).
pub fn ready(dir: PathBuf, announce: bool) -> Result<PathBuf> {
    if dir.exists() && !dir.is_dir() {
        return Err(crate::plain(format!(
            "Can't save in {}: that's a file, not a folder",
            display(&dir)
        )));
    }
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Couldn't create the folder {}", display(&dir)))?;
        if announce {
            eprintln!("{MUTED}•{RESET} Created {}", display(&dir));
        }
    }
    tempfile::NamedTempFile::new_in(&dir)
        .with_context(|| format!("Can't save in {}", display(&dir)))?;
    Ok(dir)
}

/// Write `text` to a new file named after the video. Never overwrites: a
/// second save of the same video gets " (2)" and so on.
pub fn write(dir: &Path, title: &str, text: &str) -> Result<PathBuf> {
    let base = file_name(title);
    for n in 1..1000 {
        let name = if n == 1 {
            format!("{base}.txt")
        } else {
            format!("{base} ({n}).txt")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(text.as_bytes())?;
                f.write_all(b"\n")?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(e).with_context(|| format!("Couldn't save in {}", display(dir)));
            }
        }
    }
    bail!("Couldn't find a free file name in {}", display(dir))
}

/// A path for showing, with the home folder as `~`.
pub fn display(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~/{}", rest.display())
        };
    }
    path.display().to_string()
}

/// `~` or `~/...` means the home folder. The shell does this itself
/// normally, but not when the folder is written as `--save=~/Notes`.
fn expand_home(f: &str) -> PathBuf {
    match (f.strip_prefix('~'), dirs::home_dir()) {
        (Some(""), Some(home)) => home,
        (Some(rest), Some(home)) if rest.starts_with('/') => home.join(&rest[1..]),
        _ => PathBuf::from(f),
    }
}

/// The video's title, made safe as a file name: no slashes, colons or
/// control characters, no leading dot, and not too long.
fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_start_matches('.').trim();
    let short: String = cleaned.chars().take(120).collect();
    let short = short.trim_end().to_string();
    if short.is_empty() {
        "Transcript".into()
    } else {
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_after_the_title_safely() {
        assert_eq!(
            file_name("Inside the Mind | Tim Urban: TED/2016"),
            "Inside the Mind | Tim Urban- TED-2016"
        );
        assert_eq!(file_name("  ..hidden?\n title "), "hidden? title");
        assert_eq!(file_name("..."), "Transcript");
        assert_eq!(file_name(&"a".repeat(300)).len(), 120);
    }

    #[test]
    fn makes_the_folder_and_never_overwrites() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("new").join("folder");
        let got = prepare(Some(dir.to_str().unwrap())).unwrap();
        assert!(got.is_dir());
        let first = write(&got, "A talk", "one").unwrap();
        let second = write(&got, "A talk", "two").unwrap();
        assert_eq!(first.file_name().unwrap(), "A talk.txt");
        assert_eq!(second.file_name().unwrap(), "A talk (2).txt");
        assert_eq!(std::fs::read_to_string(first).unwrap(), "one\n");
    }

    #[test]
    fn refuses_a_file_where_the_folder_should_be() {
        let base = tempfile::tempdir().unwrap();
        let file = base.path().join("notes.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(prepare(Some(file.to_str().unwrap())).is_err());
    }

    #[test]
    fn expands_the_home_folder() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/Notes"), home.join("Notes"));
        assert_eq!(expand_home("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand_home("~other/x"), PathBuf::from("~other/x"));
    }
}
