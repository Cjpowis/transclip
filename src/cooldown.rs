//! A pause on caption requests after YouTube answers one with HTTP 429 (too
//! many requests). Asking again straight away keeps the limit going, so for
//! a while transclip transcribes on this Mac instead. The pause is one small
//! file in the cache folder holding the time it ends.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How long captions are left alone after a refusal, in seconds.
pub const PAUSE_SECS: u64 = 30 * 60;

fn note() -> Option<PathBuf> {
    Some(
        dirs::cache_dir()?
            .join("transclip")
            .join("captions-paused-until"),
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whole minutes left of the pause, or None if captions can be used.
pub fn remaining() -> Option<u64> {
    remaining_at(&note()?, now())
}

/// Start, or restart, the pause. Best effort: if the note can't be written,
/// the next run simply asks YouTube again.
pub fn start() {
    if let Some(path) = note() {
        let _ = start_at(&path, now());
    }
}

fn remaining_at(path: &Path, now: u64) -> Option<u64> {
    let until: u64 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
    let left = until.checked_sub(now)?;
    // Zero means it's over; longer than a whole pause means the clock moved.
    (left > 0 && left <= PAUSE_SECS).then(|| left.div_ceil(60))
}

fn start_at(path: &Path, now: u64) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Write a new file and rename it into place, so the note is never half
    // written and a link left at that path is replaced, not followed.
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    write!(tmp, "{}", now + PAUSE_SECS)?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_and_ends() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("pause");
        assert_eq!(remaining_at(&note, 1000), None);
        start_at(&note, 1000).unwrap();
        assert_eq!(remaining_at(&note, 1000), Some(30));
        assert_eq!(remaining_at(&note, 1000 + 29 * 60 + 1), Some(1));
        assert_eq!(remaining_at(&note, 1000 + PAUSE_SECS), None);
        // Starting again restarts the full pause.
        start_at(&note, 2000).unwrap();
        assert_eq!(remaining_at(&note, 2000), Some(30));
    }

    #[test]
    fn ignores_a_damaged_or_far_future_note() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("pause");
        std::fs::write(&note, "not a time").unwrap();
        assert_eq!(remaining_at(&note, 1000), None);
        std::fs::write(&note, (1000 + PAUSE_SECS + 1).to_string()).unwrap();
        assert_eq!(remaining_at(&note, 1000), None);
    }
}
