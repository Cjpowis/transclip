//! Tidying up when a run is cut short.
//!
//! Ctrl-C, q on the live screen, closing the terminal, or the phone dropping
//! the SSH connection (which sends a hang-up signal) would otherwise end the
//! program before its temporary folder is deleted, leaving partial downloads
//! behind, or leave the terminal in full-screen mode.

use std::io::{Write, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::execute;

static PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// Set once stopping has begun.
static STOPPING: AtomicBool = AtomicBool::new(false);
/// Whether the full screen is up, so stopping knows to put it away.
static SCREEN_UP: AtomicBool = AtomicBool::new(false);
/// Held while the full screen opens, closes or draws a frame, and by
/// stopping from then until the exit, so a frame can't land on the
/// terminal after it's been put back.
static SCREEN: Mutex<()> = Mutex::new(());

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

/// On Ctrl-C, hang-up or terminate: stop.
pub fn install() {
    let _ = ctrlc::set_handler(|| stop());
}

/// The full screen has opened (true) or closed (false).
pub fn screen_up(up: bool) {
    SCREEN_UP.store(up, Ordering::SeqCst);
}

/// Open, close or draw on the full screen, taking turns with stopping. Once
/// stopping has begun this waits until the program exits.
pub fn on_screen<R>(f: impl FnOnce() -> R) -> R {
    let _turn = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    f()
}

/// Stop now: put the terminal back if the full screen is up, stop yt-dlp,
/// ffmpeg or curl if one is running (and anything they started), delete
/// what's registered, and exit.
pub fn stop() -> ! {
    if STOPPING.swap(true, Ordering::SeqCst) {
        // Already stopping on another thread, which will exit.
        loop {
            std::thread::park();
        }
    }
    // Wait for a frame being drawn to finish, then draw no more.
    let _screen = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    if SCREEN_UP.swap(false, Ordering::SeqCst) {
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();
    }
    // On the live screen Ctrl-C and q arrive as key presses, and a signal
    // from elsewhere reaches only this program, so pass it on.
    interrupt_children();
    // Let them stop writing. The work carries on meanwhile, so anything it
    // starts in that time is stopped too.
    std::thread::sleep(Duration::from_millis(300));
    interrupt_children();
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
}

/// Send Ctrl-C's signal to every program started from this one, and every
/// program they started (yt-dlp can start ffmpeg).
fn interrupt_children() {
    let all = descendants(&std::process::id().to_string());
    if !all.is_empty() {
        let _ = Command::new("kill").arg("-INT").args(&all).status();
    }
}

fn descendants(pid: &str) -> Vec<String> {
    // pgrep never lists itself.
    let children: Vec<String> = Command::new("pgrep")
        .args(["-P", pid])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut all = Vec::new();
    for child in children {
        all.extend(descendants(&child));
        all.push(child);
    }
    all
}
