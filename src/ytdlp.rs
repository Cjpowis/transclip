//! Everything that talks to yt-dlp.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::progress;

#[derive(Deserialize, Debug)]
pub struct VideoInfo {
    pub title: Option<String>,
    pub channel: Option<String>,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub language: Option<String>,
    pub webpage_url: Option<String>,
    /// Human-made captions only.
    pub subtitles: Option<HashMap<String, serde_json::Value>>,
    /// YouTube's automatic transcripts. The video's own language is listed
    /// as e.g. "en-orig"; the other languages are machine translations.
    pub automatic_captions: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "_type")]
    pub kind: Option<String>,
    /// "is_live" and "is_upcoming" are streams with nothing to transcribe yet.
    pub live_status: Option<String>,
}

fn yt_dlp() -> Command {
    let mut c = Command::new("yt-dlp");
    c.args(["--no-playlist", "--no-warnings", "--color", "never"]);
    c
}

fn not_found(e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow!("yt-dlp isn't on your PATH")
    } else {
        e.into()
    }
}

/// A yt-dlp failure. YouTube changes often and yt-dlp's fixes follow within
/// days, so if the installed yt-dlp is more than a week old, say so and give
/// the command that updates it the way it was installed.
fn failed(msg: impl std::fmt::Display, stderr: &str) -> anyhow::Error {
    match update_hint(stderr) {
        Some(hint) => anyhow!("{msg}\n  {hint}"),
        None => anyhow!("{msg}"),
    }
}

fn update_hint(stderr: &str) -> Option<String> {
    if not_a_yt_dlp_problem(stderr) {
        return None;
    }
    let real = installed_path();
    let (age, when) = if real.contains("/Cellar/yt-dlp/HEAD-") {
        // A Homebrew HEAD build reports the last release's date, not when it
        // was built, so go by when it was installed.
        let built = std::fs::metadata(&real).ok()?.modified().ok()?;
        let age = SystemTime::now().duration_since(built).ok()?.as_secs() as i64 / 86_400;
        (age, format!("was built {age} days ago"))
    } else {
        let out = Command::new("yt-dlp").arg("--version").output().ok()?;
        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (y, m, d) = release_date(&version)?;
        let today = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64 / 86_400;
        let age = today - days_from_civil(y, m, d);
        (age, format!("is from {d} {} {y}", MONTHS[(m - 1) as usize]))
    };
    if age < 7 {
        return None;
    }
    Some(format!(
        "Your yt-dlp {when}, and YouTube changes often. Try updating it: {}",
        update_command_for(&real, interpreter(&real).as_deref())
    ))
}

/// Errors about the video itself, or about the connection, which a newer
/// yt-dlp wouldn't fix.
fn not_a_yt_dlp_problem(stderr: &str) -> bool {
    const SIGNS: [&str; 18] = [
        "private video",
        "video unavailable",
        "this video is unavailable",
        "has been removed",
        "members-only",
        "join this channel",
        "sign in to confirm your age",
        "not available in your country",
        "blocked it in your country",
        "copyright",
        "premieres in",
        "unsupported url",
        "is not a valid url",
        "failed to resolve",
        "nodename nor servname",
        "name or service not known",
        "network is unreachable",
        "timed out",
    ];
    let s = stderr.to_lowercase();
    SIGNS.iter().any(|sign| s.contains(sign))
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// yt-dlp versions are release dates: 2026.11.16, or 2026.11.16.232655.dev0
/// for a nightly.
fn release_date(version: &str) -> Option<(i64, i64, i64)> {
    let mut parts = version.split('.');
    let y = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Days since 1970-01-01 for a calendar date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Where the yt-dlp on the PATH really lives, with symlinks resolved.
fn installed_path() -> String {
    let found = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("yt-dlp"))
            .find(|p| p.is_file())
    });
    found
        .and_then(|p| std::fs::canonicalize(p).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The Python a pip-installed yt-dlp script runs with, from its first line.
/// None for the standalone build (which uses /usr/bin/env) or a binary.
fn interpreter(path: &str) -> Option<String> {
    use std::io::Read;
    let mut head = [0u8; 256];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let first = String::from_utf8_lossy(&head[..n]);
    let program = first
        .strip_prefix("#!")?
        .lines()
        .next()?
        .split_whitespace()
        .next()?;
    (program.starts_with('/') && program.contains("python") && !program.ends_with("/env"))
        .then(|| program.to_string())
}

fn update_command_for(real_path: &str, python: Option<&str>) -> String {
    if real_path.contains("/Cellar/yt-dlp/HEAD-") {
        "brew upgrade --fetch-HEAD yt-dlp".into()
    } else if real_path.contains("/Cellar/yt-dlp/") {
        "brew upgrade yt-dlp".into()
    } else if real_path.contains("/uv/tools/") {
        "uv tool upgrade yt-dlp".into()
    } else if real_path.contains("/pipx/") {
        "pipx upgrade yt-dlp".into()
    } else if let Some(python) = python {
        format!("{python} -m pip install -U \"yt-dlp[default]\"")
    } else {
        "yt-dlp -U".into()
    }
}

/// The last couple of meaningful lines of yt-dlp's error output.
fn last_lines(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    let lines: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(2)..];
    tail.join(" / ").replace("ERROR: ", "")
}

pub fn probe(url: &str) -> Result<VideoInfo> {
    let out = yt_dlp()
        // --flat-playlist: for a playlist or channel link, just list it
        // (so it can be refused quickly) instead of looking up every video.
        .args(["-J", "--skip-download", "--flat-playlist", "--", url])
        .output()
        .map_err(not_found)?;
    if !out.status.success() {
        return Err(failed(
            format!(
                "yt-dlp couldn't read that link: {}",
                last_lines(&out.stderr)
            ),
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    serde_json::from_slice(&out.stdout).context("yt-dlp gave unexpected output")
}

/// Pick a human-made caption track: the wanted language first (exact, then
/// regional variants like en-GB), then the video's own language.
pub fn pick_caption_lang(info: &VideoInfo, want: &str) -> Option<String> {
    let subs = info.subtitles.as_ref()?;
    let keys: Vec<&String> = subs.keys().filter(|k| k.as_str() != "live_chat").collect();
    let find = |base: &str| -> Option<String> {
        if let Some(k) = keys.iter().find(|k| k.as_str() == base) {
            return Some((*k).clone());
        }
        let prefix = format!("{base}-");
        let mut variants: Vec<&String> = keys
            .iter()
            .copied()
            .filter(|k| k.starts_with(&prefix))
            .collect();
        variants.sort();
        variants.first().map(|k| (*k).clone())
    };
    find(want).or_else(|| {
        let lang = info.language.as_deref()?;
        let base = lang.split('-').next().unwrap_or(lang);
        if base == want { None } else { find(base) }
    })
}

pub fn download_captions(url: &str, lang: &str, dir: &Path) -> Result<PathBuf> {
    let out = yt_dlp()
        .args(["--skip-download", "--write-subs", "--sub-langs", lang])
        .args(["--sub-format", "json3", "-o"])
        .arg(dir.join("captions.%(ext)s"))
        .args(["--", url])
        .output()
        .map_err(not_found)?;
    if !out.status.success() {
        bail!("{}", last_lines(&out.stderr));
    }
    find_file(dir, "captions.", |n| n.ends_with(".json3"))
        .ok_or_else(|| anyhow!("no json3 captions were saved"))
}

/// Pick YouTube's automatic transcript in the video's own language (the
/// "-orig" track), not a machine translation of it. Falls back to the
/// wanted language when YouTube doesn't mark an original.
pub fn pick_auto_caption_lang(info: &VideoInfo, want: &str) -> Option<String> {
    let auto = info.automatic_captions.as_ref()?;
    let mut originals: Vec<&String> = auto.keys().filter(|k| k.ends_with("-orig")).collect();
    originals.sort();
    if let Some(k) = originals.first() {
        return Some((*k).clone());
    }
    auto.contains_key(want).then(|| want.to_string())
}

pub fn download_auto_captions(url: &str, lang: &str, dir: &Path) -> Result<PathBuf> {
    let out = yt_dlp()
        .args(["--skip-download", "--write-auto-subs", "--sub-langs", lang])
        .args(["--sub-format", "json3", "-o"])
        .arg(dir.join("auto.%(ext)s"))
        .args(["--", url])
        .output()
        .map_err(not_found)?;
    if !out.status.success() {
        bail!("{}", last_lines(&out.stderr));
    }
    find_file(dir, "auto.", |n| n.ends_with(".json3"))
        .ok_or_else(|| anyhow!("no json3 transcript was saved"))
}

/// Download the smallest audio-only stream, showing size and speed as it goes.
///
/// The model only hears 16 kHz mono, so the top-quality track is wasted
/// download. We pick the original-language track (not a dubbed one), prefer
/// Opus because it holds speech up well at low bitrates, then take the
/// lowest bitrate: on YouTube that's format 249, about 50 kbps, roughly a
/// third the size of the default.
pub fn download_audio(url: &str, dir: &Path) -> Result<PathBuf> {
    const TEMPLATE: &str = "download:YTT|%(progress.downloaded_bytes)s|%(progress.total_bytes)s\
                            |%(progress.total_bytes_estimate)s|%(progress.speed)s";
    let mut child = yt_dlp()
        .args([
            "-f",
            "ba/b",
            "-S",
            "lang,acodec:opus,+abr",
            "--newline",
            "--progress-template",
            TEMPLATE,
            "-o",
        ])
        .arg(dir.join("audio.%(ext)s"))
        .args(["--", url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(not_found)?;

    let mut stderr = child.stderr.take().expect("piped stderr");
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let bar = progress::bar("Downloading ");
    // yt-dlp can take a little while before the first bytes arrive.
    bar.set_prefix("starting…");
    let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    for line in stdout.lines().map_while(Result::ok) {
        let Some(rest) = line.strip_prefix("YTT|") else {
            continue;
        };
        let f: Vec<&str> = rest.split('|').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok());
        let Some(got) = num(0) else { continue };
        let total = num(1).or(num(2)).filter(|t| *t > 0.0);
        if let Some(t) = total {
            bar.set_position(((got / t) * 1000.0).min(1000.0) as u64);
        }
        let mut info = match total {
            Some(t) => format!("{:.1}/{:.1} MB", got / 1e6, t / 1e6),
            None => format!("{:.1} MB", got / 1e6),
        };
        if let Some(speed) = num(3) {
            info.push_str(&format!(" · {:.1} MB/s", speed / 1e6));
        }
        bar.set_prefix(info);
    }

    let status = child.wait()?;
    let err = err_reader.join().unwrap_or_default();
    if !status.success() {
        bar.finish_and_clear();
        return Err(failed(
            format!(
                "Couldn't download the audio: {}",
                last_lines(err.as_bytes())
            ),
            &err,
        ));
    }
    let Some(path) = find_file(dir, "audio.", |n| {
        !n.ends_with(".part") && !n.ends_with(".ytdl")
    }) else {
        bar.finish_and_clear();
        bail!("yt-dlp finished but no audio file was saved");
    };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    progress::done(
        &bar,
        format!("Downloaded audio ({:.1} MB)", size as f64 / 1e6),
    );
    Ok(path)
}

fn find_file(dir: &Path, prefix: &str, ok: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && ok(n))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(langs: &[&str], video_lang: Option<&str>) -> VideoInfo {
        VideoInfo {
            title: None,
            channel: None,
            uploader: None,
            duration: None,
            language: video_lang.map(String::from),
            webpage_url: None,
            subtitles: Some(
                langs
                    .iter()
                    .map(|l| (l.to_string(), serde_json::Value::Null))
                    .collect(),
            ),
            automatic_captions: None,
            kind: None,
            live_status: None,
        }
    }

    fn auto(langs: &[&str]) -> VideoInfo {
        let mut i = info(&[], None);
        i.automatic_captions = Some(
            langs
                .iter()
                .map(|l| (l.to_string(), serde_json::Value::Null))
                .collect(),
        );
        i
    }

    #[test]
    fn reads_yt_dlp_release_dates() {
        assert_eq!(release_date("2026.11.16"), Some((2026, 11, 16)));
        assert_eq!(release_date("2026.11.16.232655.dev0"), Some((2026, 11, 16)));
        assert_eq!(release_date("rubbish"), None);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            days_from_civil(2026, 11, 17) - days_from_civil(2026, 10, 26),
            22
        );
    }

    #[test]
    fn suggests_the_right_update_command() {
        let brew = "/opt/homebrew/Cellar/yt-dlp/2026.10.26/libexec/bin/yt-dlp";
        assert_eq!(update_command_for(brew, None), "brew upgrade yt-dlp");
        let head = "/opt/homebrew/Cellar/yt-dlp/HEAD-abc123/libexec/bin/yt-dlp";
        assert_eq!(
            update_command_for(head, None),
            "brew upgrade --fetch-HEAD yt-dlp"
        );
        let uv = "/Users/me/.local/share/uv/tools/yt-dlp/bin/yt-dlp";
        let uv_python = Some("/Users/me/.local/share/uv/tools/yt-dlp/bin/python");
        assert_eq!(update_command_for(uv, uv_python), "uv tool upgrade yt-dlp");
        let venv = "/Users/me/venv/bin/yt-dlp";
        assert_eq!(
            update_command_for(venv, Some("/Users/me/venv/bin/python3")),
            "/Users/me/venv/bin/python3 -m pip install -U \"yt-dlp[default]\""
        );
        assert_eq!(
            update_command_for("/usr/local/bin/yt-dlp", None),
            "yt-dlp -U"
        );
    }

    #[test]
    fn reads_the_python_a_script_runs_with() {
        let dir = tempfile::tempdir().unwrap();
        let pip = dir.path().join("pip-yt-dlp");
        std::fs::write(&pip, "#!/Users/me/venv/bin/python3\nimport yt_dlp\n").unwrap();
        assert_eq!(
            interpreter(pip.to_str().unwrap()).as_deref(),
            Some("/Users/me/venv/bin/python3")
        );
        let zip = dir.path().join("zip-yt-dlp");
        std::fs::write(&zip, "#!/usr/bin/env python3\nPK...").unwrap();
        assert_eq!(interpreter(zip.to_str().unwrap()), None);
        let bin = dir.path().join("bin-yt-dlp");
        std::fs::write(&bin, [0xcf, 0xfa, 0xed, 0xfe, 0, 1, 2]).unwrap();
        assert_eq!(interpreter(bin.to_str().unwrap()), None);
    }

    #[test]
    fn leaves_out_the_hint_when_updating_would_not_help() {
        assert!(not_a_yt_dlp_problem(
            "ERROR: [youtube] abc: Private video. Sign in if you've been granted access"
        ));
        assert!(not_a_yt_dlp_problem(
            "ERROR: [youtube] abc: Video unavailable"
        ));
        assert!(not_a_yt_dlp_problem(
            "ERROR: Unable to download webpage: <urlopen error [Errno 8] nodename nor servname provided>"
        ));
        assert!(!not_a_yt_dlp_problem(
            "ERROR: unable to download video data: HTTP Error 403: Forbidden"
        ));
    }

    #[test]
    fn picks_the_original_language_transcript_not_a_translation() {
        assert_eq!(
            pick_auto_caption_lang(&auto(&["de", "en", "en-orig", "fr"]), "en").as_deref(),
            Some("en-orig")
        );
        assert_eq!(
            pick_auto_caption_lang(&auto(&["en", "fr", "fr-orig"]), "en").as_deref(),
            Some("fr-orig")
        );
        assert_eq!(
            pick_auto_caption_lang(&auto(&["de", "en"]), "en").as_deref(),
            Some("en")
        );
        assert_eq!(pick_auto_caption_lang(&auto(&["de"]), "en"), None);
        assert_eq!(pick_auto_caption_lang(&info(&[], None), "en"), None);
    }

    #[test]
    fn picks_exact_then_variant_then_video_language() {
        assert_eq!(
            pick_caption_lang(&info(&["fr", "en"], None), "en").as_deref(),
            Some("en")
        );
        assert_eq!(
            pick_caption_lang(&info(&["en-US", "en-GB"], None), "en").as_deref(),
            Some("en-GB")
        );
        assert_eq!(
            pick_caption_lang(&info(&["fr"], Some("fr")), "en").as_deref(),
            Some("fr")
        );
        assert_eq!(pick_caption_lang(&info(&["de"], Some("fr")), "en"), None);
        assert_eq!(pick_caption_lang(&info(&["live_chat"], None), "en"), None);
    }
}
