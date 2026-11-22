//! Everything that talks to yt-dlp.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::show::{Show, Stage};

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
    pub chapters: Option<Vec<ChapterInfo>>,
}

#[derive(Deserialize, Debug)]
pub struct ChapterInfo {
    pub start_time: Option<f64>,
    pub title: Option<String>,
}

/// What a link points at, where that can be told from the link alone
/// (asking YouTube about a playlist takes several seconds).
#[derive(Debug, PartialEq)]
pub enum Link {
    /// A YouTube Mix: an endless playlist that starts from one video. The
    /// Mix's ID is "RD" and that video's ID, which this holds.
    Mix(String),
    /// A playlist with no one video to pick.
    Playlist,
    /// Anything else, a video or not, for yt-dlp to look up.
    Other,
}

pub fn read_link(link: &str) -> Link {
    let link = link.split('#').next().unwrap_or("");
    let (place, query) = link.split_once('?').unwrap_or((link, ""));
    let place = place.trim_end_matches('/');
    let on_youtube = place.contains("youtube.com/") || place.contains("youtube-nocookie.com/");
    // A link with a video in it (watch?v=…&list=…) is that video; yt-dlp is
    // told to ignore the list.
    if !on_youtube || !place.ends_with("/playlist") {
        return Link::Other;
    }
    let list = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("list="))
        .unwrap_or("");
    let video = list
        .strip_prefix("RDAMVM")
        .or_else(|| list.strip_prefix("RD"))
        .filter(|id| {
            id.len() == 11
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
    match video {
        Some(id) => Link::Mix(id.to_string()),
        None => Link::Playlist,
    }
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
    const SIGNS: [&str; 20] = [
        "http error 429",
        "too many requests",
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

/// The installed yt-dlp's version (None if it isn't found) and the command
/// that updates it, for `--report`.
pub fn installed() -> (Option<String>, String) {
    let real = installed_path();
    let version = Command::new("yt-dlp")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty());
    (
        version,
        update_command_for(&real, interpreter(&real).as_deref()),
    )
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

/// YouTube refused the request for coming too often (HTTP 429).
pub fn too_many_requests(e: &anyhow::Error) -> bool {
    let s = format!("{e:#}").to_lowercase();
    s.contains("http error 429") || s.contains("too many requests")
}

/// Where a download starts. `Lookup` is the answer `probe` got, saved to a
/// file: yt-dlp downloads straight away instead of asking YouTube about the
/// whole video again. `Link` makes a fresh lookup.
#[derive(Clone, Copy)]
pub enum Origin<'a> {
    Link(&'a str),
    Lookup(&'a Path),
}

impl Origin<'_> {
    fn add_to(self, cmd: &mut Command) {
        match self {
            Origin::Link(url) => cmd.args(["--", url]),
            Origin::Lookup(json) => cmd.arg("--load-info-json").arg(json),
        };
    }
}

/// Look the video up once. Returns what we need from the answer, and the
/// answer itself so the downloads can reuse it (see `Origin::Lookup`).
pub fn probe(url: &str) -> Result<(VideoInfo, Vec<u8>)> {
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
    let info = serde_json::from_slice(&out.stdout).context("yt-dlp gave unexpected output")?;
    Ok((info, out.stdout))
}

/// Pick a human-made caption track: the wanted language first (exact, then
/// regional variants like en-GB), then the video's own language, or English
/// when that isn't known.
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
    // In YouTube's own codes too, like "iw" for Hebrew ("he").
    let yt_want = base_lang(want);
    let found = find(want).or_else(|| {
        if yt_want == want {
            None
        } else {
            find(&yt_want)
        }
    });
    found.or_else(|| match info.language.as_deref() {
        Some(lang) => {
            let base = base_lang(lang);
            if base == yt_want { None } else { find(&base) }
        }
        // With no language to go on, English captions written by a person
        // still beat this Mac's model, which only does English anyway.
        None if yt_want != "en" => find("en"),
        None => None,
    })
}

pub fn download_captions(from: Origin, lang: &str, dir: &Path) -> Result<PathBuf> {
    let mut cmd = yt_dlp();
    cmd.args(["--skip-download", "--write-subs", "--sub-langs", lang])
        .args(["--sub-format", "json3", "-o"])
        .arg(dir.join("captions.%(ext)s"));
    from.add_to(&mut cmd);
    let out = cmd.output().map_err(not_found)?;
    if !out.status.success() {
        bail!("{}", last_lines(&out.stderr));
    }
    find_file(dir, "captions.", |n| n.ends_with(".json3"))
        .ok_or_else(|| anyhow!("no json3 captions were saved"))
}

/// Pick YouTube's automatic transcript in the language the video is spoken
/// in, not a machine translation of it.
///
/// YouTube marks that transcript "-orig" (e.g. "en-orig"). A video it has
/// dubbed with AI voices has an "-orig" transcript for every dubbed voice
/// too, so the right one is in the video's own language, which yt-dlp reads
/// from the original audio track. If yt-dlp doesn't know that language, a
/// lone "-orig" is taken, and of several only one in the wanted language is
/// trusted. Falls back to the wanted language when nothing is marked
/// "-orig".
pub fn pick_auto_caption_lang(info: &VideoInfo, want: &str) -> Option<String> {
    let auto = info.automatic_captions.as_ref()?;
    let mut originals: Vec<&str> = auto
        .keys()
        .filter_map(|k| k.strip_suffix("-orig"))
        .collect();
    originals.sort();
    if originals.is_empty() {
        return auto.contains_key(want).then(|| want.to_string());
    }
    let pick = match info.language.as_deref() {
        // The video's own language, or nothing rather than a dub.
        Some(lang) => matching(&originals, lang),
        None if originals.len() == 1 => Some(originals[0]),
        None => matching(&originals, want),
    };
    pick.map(|l| format!("{l}-orig"))
}

/// The track in `lang` among `langs`: exactly, or else the first in the same
/// language whatever its region or script ("zh-Hans" for "zh-CN").
fn matching<'a>(langs: &[&'a str], lang: &str) -> Option<&'a str> {
    let wanted = base_lang(lang);
    langs
        .iter()
        .find(|l| **l == lang)
        .or_else(|| langs.iter().find(|l| base_lang(l) == wanted))
        .copied()
}

/// A language without its region or script, in the codes YouTube uses for
/// transcripts: "zh" for "zh-Hans", and the older "iw" for Hebrew ("he")
/// and "no" for Norwegian ("nb").
fn base_lang(code: &str) -> String {
    let base = code
        .split(['-', '_'])
        .next()
        .unwrap_or(code)
        .to_ascii_lowercase();
    match base.as_str() {
        "he" => "iw".to_string(),
        "nb" => "no".to_string(),
        _ => base,
    }
}

pub fn download_auto_captions(from: Origin, lang: &str, dir: &Path) -> Result<PathBuf> {
    let mut cmd = yt_dlp();
    cmd.args(["--skip-download", "--write-auto-subs", "--sub-langs", lang])
        .args(["--sub-format", "json3", "-o"])
        .arg(dir.join("auto.%(ext)s"));
    from.add_to(&mut cmd);
    let out = cmd.output().map_err(not_found)?;
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
pub fn download_audio(from: Origin, dir: &Path, show: &Show) -> Result<PathBuf> {
    const TEMPLATE: &str = "download:YTT|%(progress.downloaded_bytes)s|%(progress.total_bytes)s\
                            |%(progress.total_bytes_estimate)s|%(progress.speed)s";
    let mut cmd = yt_dlp();
    cmd.args([
        "-f",
        "ba/b",
        "-S",
        "lang,acodec:opus,+abr",
        "--newline",
        "--progress-template",
        TEMPLATE,
        "-o",
    ])
    .arg(dir.join("audio.%(ext)s"));
    from.add_to(&mut cmd);
    let mut child = cmd
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

    show.start(Stage::Audio, "Downloading ");
    let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    for line in stdout.lines().map_while(Result::ok) {
        let Some(rest) = line.strip_prefix("YTT|") else {
            continue;
        };
        let f: Vec<&str> = rest.split('|').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok());
        let Some(got) = num(0) else { continue };
        let total = num(1).or(num(2)).filter(|t| *t > 0.0);
        let share = total.map(|t| (got / t).min(1.0));
        let mut info = match total {
            Some(t) => format!("{:.1}/{:.1} MB", got / 1e6, t / 1e6),
            None => format!("{:.1} MB", got / 1e6),
        };
        if let Some(speed) = num(3) {
            info.push_str(&format!(" · {:.1} MB/s", speed / 1e6));
        }
        show.progress(Stage::Audio, share, &info);
    }

    let status = child.wait()?;
    let err = err_reader.join().unwrap_or_default();
    if !status.success() {
        show.clear();
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
        show.clear();
        bail!("yt-dlp finished but no audio file was saved");
    };
    let mb = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) as f64 / 1e6;
    show.finish(
        Stage::Audio,
        Some(format!("Downloaded audio ({mb:.1} MB)")),
        &format!("{mb:.1} MB"),
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

    #[test]
    fn a_mix_link_gives_the_video_it_starts_from() {
        let mix = Link::Mix("JWoYy4Ah81s".into());
        for link in [
            "https://youtube.com/playlist?list=RDJWoYy4Ah81s&playnext=1&si=6QxaM3NmoFJNiU-1",
            "https://www.youtube.com/playlist?list=RDJWoYy4Ah81s",
            "https://music.youtube.com/playlist?list=RDAMVMJWoYy4Ah81s",
            "https://m.youtube.com/playlist/?si=x&list=RDJWoYy4Ah81s#top",
        ] {
            assert_eq!(read_link(link), mix, "{link}");
        }
    }

    #[test]
    fn a_playlist_link_is_known_without_asking_youtube() {
        for link in [
            "https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG",
            "https://youtube.com/playlist?list=OLAK5uy_kDk9xu6YkU8vV4R6ZbYc7aK9eb0hYrwQs",
            // An album radio and My Mix: no one video to start from.
            "https://music.youtube.com/playlist?list=RDCLAK5uy_kmPRjHDECIcuVwnKsx2Ng7fyNgFKWNJFs",
            "https://www.youtube.com/playlist?list=RDMM",
            "https://www.youtube.com/playlist",
        ] {
            assert_eq!(read_link(link), Link::Playlist, "{link}");
        }
    }

    #[test]
    fn a_link_with_a_video_in_it_is_left_alone() {
        for link in [
            "https://www.youtube.com/watch?v=JWoYy4Ah81s&list=RDJWoYy4Ah81s&start_radio=1",
            "https://youtu.be/JWoYy4Ah81s?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG",
            "https://www.youtube.com/shorts/JWoYy4Ah81s",
            "https://example.com/playlist?list=RDJWoYy4Ah81s",
            "JWoYy4Ah81s",
        ] {
            assert_eq!(read_link(link), Link::Other, "{link}");
        }
    }

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
            chapters: None,
        }
    }

    fn auto(langs: &[&str]) -> VideoInfo {
        auto_in(langs, None)
    }

    fn auto_in(langs: &[&str], video_lang: Option<&str>) -> VideoInfo {
        let mut i = info(&[], video_lang);
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
        assert!(not_a_yt_dlp_problem(
            "ERROR: Unable to download video subtitles for 'en': HTTP Error 429: Too Many Requests"
        ));
        assert!(!not_a_yt_dlp_problem(
            "ERROR: unable to download video data: HTTP Error 403: Forbidden"
        ));
    }

    #[test]
    fn spots_youtube_refusing_too_many_requests() {
        assert!(too_many_requests(&anyhow!(
            "Unable to download video subtitles for 'en': HTTP Error 429: Too Many Requests"
        )));
        assert!(!too_many_requests(&anyhow!(
            "unable to download video data: HTTP Error 403: Forbidden"
        )));
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
        // A lone original in another language than the video's audio is a
        // dub's, so it isn't used.
        assert_eq!(
            pick_auto_caption_lang(&auto_in(&["en", "es-orig"], Some("en")), "en"),
            None
        );
        assert_eq!(
            pick_auto_caption_lang(&auto_in(&["en-orig"], Some("en-GB")), "fr").as_deref(),
            Some("en-orig")
        );
    }

    #[test]
    fn matches_languages_written_differently() {
        let zh = auto_in(&["ar-orig", "zh-Hans", "zh-Hans-orig"], Some("zh-CN"));
        assert_eq!(
            pick_auto_caption_lang(&zh, "en").as_deref(),
            Some("zh-Hans-orig")
        );
        let he = auto_in(&["ar-orig", "iw-orig"], Some("he"));
        assert_eq!(
            pick_auto_caption_lang(&he, "en").as_deref(),
            Some("iw-orig")
        );
        let pt = auto_in(&["ar-orig", "pt-orig"], Some("pt-BR"));
        assert_eq!(
            pick_auto_caption_lang(&pt, "en").as_deref(),
            Some("pt-orig")
        );
    }

    /// The layout of a real dubbed video (-0uvPVwPFgM, November 2026): an
    /// "-orig" transcript for each AI-dubbed voice as well as the speaker's.
    /// Taking the first alphabetically gave the Arabic dub.
    #[test]
    fn picks_the_speakers_transcript_on_a_dubbed_video() {
        let dubbed = [
            "ar-orig", "ar", "bn-orig", "de-orig", "en", "en-orig", "fr-orig", "hi-orig",
            "iw-orig", "ja-orig", "nl-orig", "pt-orig",
        ];
        let video = auto_in(&dubbed, Some("en-US"));
        assert_eq!(
            pick_auto_caption_lang(&video, "en").as_deref(),
            Some("en-orig")
        );
        // Someone whose language is French still gets what was said.
        assert_eq!(
            pick_auto_caption_lang(&video, "fr").as_deref(),
            Some("en-orig")
        );
        // An exact match wins over the language alone.
        let exact = auto_in(&["en-US-orig", "en-orig", "fr-orig"], Some("en-US"));
        assert_eq!(
            pick_auto_caption_lang(&exact, "en").as_deref(),
            Some("en-US-orig")
        );
        // The video's language has no transcript: no dub is used instead.
        let missing = auto_in(&["ar-orig", "fr-orig"], Some("es"));
        assert_eq!(pick_auto_caption_lang(&missing, "fr"), None);
        // Language unknown: only one in the wanted language is trusted.
        let unknown = auto(&dubbed);
        assert_eq!(
            pick_auto_caption_lang(&unknown, "en").as_deref(),
            Some("en-orig")
        );
        assert_eq!(pick_auto_caption_lang(&unknown, "es"), None);
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
        // Someone reading French, a video of unknown language: English
        // captions written by a person are still used.
        assert_eq!(
            pick_caption_lang(&info(&["en"], None), "fr").as_deref(),
            Some("en")
        );
        assert_eq!(pick_caption_lang(&info(&["de"], None), "fr"), None);
        // YouTube's older code for Hebrew, whether it's the wanted language
        // or the video's.
        assert_eq!(
            pick_caption_lang(&info(&["en", "iw"], None), "he").as_deref(),
            Some("iw")
        );
        assert_eq!(
            pick_caption_lang(&info(&["iw"], Some("he")), "en").as_deref(),
            Some("iw")
        );
    }
}
