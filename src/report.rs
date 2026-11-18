//! `transclip --report`: a short description of this setup and the last
//! problem, to paste into a GitHub issue. The home folder's path (it holds
//! the username), links and YouTube video IDs are left out.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const NEW_ISSUE: &str = "https://github.com/Cjpowis/transclip/issues/new";

fn last_error_file() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("transclip").join("last-error"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Keep a failed run's message for the next report. Best effort.
pub fn record_error(msg: &str) {
    let Some(path) = last_error_file() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, format!("{}\n{}", now(), scrub(msg)));
}

/// The report, ready to paste into an issue.
pub fn build() -> String {
    let mut lines = vec![
        "**Transclip report**".to_string(),
        format!("- Transclip {}", env!("CARGO_PKG_VERSION")),
        format!("- macOS {}", macos().unwrap_or_else(|| "(unknown)".into())),
    ];
    let (version, update) = crate::ytdlp::installed();
    lines.push(match version {
        Some(v) => format!("- yt-dlp {v}, updated with `{update}`"),
        None => "- yt-dlp: not found".into(),
    });
    lines.push(match ffmpeg() {
        Some(v) => format!("- ffmpeg {v}"),
        None => "- ffmpeg: not found".into(),
    });
    lines.push(match crate::transcribe::model_size() {
        Some(bytes) => format!("- Speech model: downloaded ({} MB)", bytes / 1_000_000),
        None => "- Speech model: not downloaded".into(),
    });
    if let Some(mins) = crate::cooldown::remaining() {
        lines.push(format!(
            "- Captions paused for {mins} more min (YouTube said too many requests)"
        ));
    }
    lines.push(match last_error() {
        Some((age, msg)) => format!("- Last problem ({age}):\n\n```\n{msg}\n```"),
        None => "- Last problem: none recorded".into(),
    });
    scrub(&lines.join("\n"))
}

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// "26.1 (25B78), Apple M2"
fn macos() -> Option<String> {
    let version = output("sw_vers", &["-productVersion"])?;
    let build = output("sw_vers", &["-buildVersion"])
        .map(|b| format!(" ({b})"))
        .unwrap_or_default();
    let chip = output("sysctl", &["-n", "machdep.cpu.brand_string"])
        .map(|c| format!(", {c}"))
        .unwrap_or_default();
    Some(format!("{version}{build}{chip}"))
}

/// From "ffmpeg version 8.0 Copyright (c) ...".
fn ffmpeg() -> Option<String> {
    let text = output("ffmpeg", &["-hide_banner", "-version"])?;
    text.lines()
        .next()?
        .split_whitespace()
        .nth(2)
        .map(str::to_string)
}

fn last_error() -> Option<(String, String)> {
    let text = std::fs::read_to_string(last_error_file()?).ok()?;
    let (when, msg) = text.split_once('\n')?;
    let when: u64 = when.trim().parse().ok()?;
    Some((ago(now().saturating_sub(when)), msg.trim().to_string()))
}

fn ago(secs: u64) -> String {
    match secs {
        0..120 => "just now".into(),
        120..5_400 => format!("{} minutes ago", secs / 60),
        5_400..172_800 => format!("{} hours ago", (secs + 1_800) / 3_600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Leave out what could identify the person: the home folder's path,
/// links, and YouTube video IDs.
pub fn scrub(s: &str) -> String {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    scrub_with(s, home.as_deref())
}

fn scrub_with(s: &str, home: Option<&str>) -> String {
    let s = match home {
        Some(h) if h.len() > 1 => s.replace(h, "~"),
        _ => s.to_string(),
    };
    hide_video_ids(&hide_links(&s))
}

fn hide_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("http") {
        let (before, from) = rest.split_at(i);
        out.push_str(before);
        if from.starts_with("https://") || from.starts_with("http://") {
            let end = from.find(char::is_whitespace).unwrap_or(from.len());
            out.push_str("(link)");
            rest = &from[end..];
        } else {
            out.push_str("http");
            rest = &from[4..];
        }
    }
    out.push_str(rest);
    out
}

/// yt-dlp names the video in its messages: "[youtube] dQw4w9WgXcQ: ...".
fn hide_video_ids(s: &str) -> String {
    const TAG: &str = "[youtube] ";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(TAG) {
        let (before, from) = rest.split_at(i + TAG.len());
        out.push_str(before);
        let id_len = from
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(from.len());
        if id_len == 11 {
            out.push_str("(video)");
            rest = &from[11..];
        } else {
            rest = from;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_out_the_home_folder_links_and_video_ids() {
        let msg = "yt-dlp couldn't read that link: [youtube] dQw4w9WgXcQ: Video unavailable\n  \
                   Try: /Users/sam/venv/bin/python3 -m pip install -U \"yt-dlp[default]\"\n\
                   Unsupported URL: https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=5 (ok)";
        let got = scrub_with(msg, Some("/Users/sam"));
        assert_eq!(
            got,
            "yt-dlp couldn't read that link: [youtube] (video): Video unavailable\n  \
             Try: ~/venv/bin/python3 -m pip install -U \"yt-dlp[default]\"\n\
             Unsupported URL: (link) (ok)"
        );
        assert!(!got.contains("sam") && !got.contains("dQw4w9WgXcQ"));
    }

    #[test]
    fn keeps_ordinary_words() {
        assert_eq!(
            scrub_with("httpd and [youtube] search: no", Some("/Users/sam")),
            "httpd and [youtube] search: no"
        );
    }

    #[test]
    fn says_how_long_ago() {
        assert_eq!(ago(30), "just now");
        assert_eq!(ago(10 * 60), "10 minutes ago");
        assert_eq!(ago(3 * 3_600), "3 hours ago");
        assert_eq!(ago(3 * 86_400), "3 days ago");
    }
}
