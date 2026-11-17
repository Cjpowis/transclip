//! Tidying up when a run is cut short.
//!
//! Ctrl-C, closing the terminal, or the phone dropping the SSH connection
//! (which sends a hang-up signal) would otherwise end the program before its
//! temporary folder is deleted, leaving partial downloads behind.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

static PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Delete `path` if the run is interrupted before it's unregistered.
pub fn register(path: &Path) {
    if let Ok(mut paths) = PATHS.lock() {
        paths.push(path.to_path_buf());
    }
}

pub fn unregister(path: &Path) {
    if let Ok(mut paths) = PATHS.lock() {
        paths.retain(|p| p != path);
    }
}

/// On Ctrl-C, hang-up or terminate: delete what's registered and exit.
pub fn install() {
    let _ = ctrlc::set_handler(|| {
        // yt-dlp, ffmpeg and curl get the same signal; let them stop writing.
        std::thread::sleep(Duration::from_millis(300));
        if let Ok(paths) = PATHS.lock() {
            for p in paths.iter() {
                let _ = if p.is_dir() {
                    std::fs::remove_dir_all(p)
                } else {
                    std::fs::remove_file(p)
                };
            }
        }
        // Spinners hide the cursor, so show it again. After a hang-up there's
        // no terminal, so ignore errors.
        let _ = std::io::stderr().write_all(b"\x1b[?25h\n");
        std::process::exit(130);
    });
}
