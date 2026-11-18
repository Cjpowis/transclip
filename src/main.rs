mod captions;
mod cleanup;
mod clipboard;
mod cooldown;
mod progress;
mod report;
mod text;
mod transcribe;
mod viewer;
mod ytdlp;

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{Result, bail};
use clap::Parser;

use crate::progress::{AMBER, GREEN, MUTED, RED, RESET};
use crate::text::Timed;

/// Get the transcript of a YouTube video.
///
/// Tries, in order: the video's human-written captions, YouTube's
/// automatic transcript (if it's punctuated), then transcribing the audio
/// on this Mac with NVIDIA Parakeet. The transcript is copied to your
/// clipboard (over SSH too, through your terminal app) and shown in a
/// scrollable viewer. Only the speech model is kept on disk, plus a note of
/// when to try captions again if YouTube says they were asked for too often.
#[derive(Parser, Debug)]
#[command(name = "transclip", version, about, long_about)]
struct Args {
    /// Link to the video (leave it off and you'll be asked to paste it)
    url: Option<String>,

    /// Caption language to look for first
    #[arg(long, default_value = "en", value_name = "CODE")]
    lang: String,

    /// Skip YouTube's captions and transcript and always transcribe on
    /// this Mac (most accurate for names, numbers and commands)
    #[arg(short, long)]
    transcribe: bool,

    /// Print the transcript instead of opening the viewer
    #[arg(short, long)]
    print: bool,

    /// Don't copy to the clipboard automatically
    #[arg(long)]
    no_copy: bool,

    /// Leave the title and link off the top of the copied text
    #[arg(long)]
    text_only: bool,

    /// Delete the downloaded speech model (about 650 MB) and exit. It
    /// downloads again the next time it's needed.
    #[arg(long)]
    delete_model: bool,

    /// Copy a short report for a GitHub issue: versions, setup and the last
    /// problem (no links, video IDs or your folder names)
    #[arg(long)]
    report: bool,
}

pub enum Source {
    Captions(String),
    AutoCaptions(String),
    Parakeet,
}

pub struct Transcript {
    pub title: String,
    pub channel: Option<String>,
    pub url: String,
    pub duration: Option<f64>,
    pub source: Source,
    pub paragraphs: Vec<String>,
}

impl Transcript {
    pub fn body(&self) -> String {
        self.paragraphs.join("\n\n")
    }

    /// What goes on the clipboard: title and link on top unless asked not to.
    pub fn clipboard_text(&self, text_only: bool) -> String {
        if text_only {
            self.body()
        } else {
            format!("{}\n{}\n\n{}", self.title, self.url, self.body())
        }
    }

    pub fn source_label(&self) -> String {
        match &self.source {
            Source::Captions(lang) => format!("Human-written captions ({lang})"),
            Source::AutoCaptions(lang) => format!("YouTube automatic transcript ({lang})"),
            Source::Parakeet => "Transcribed on this Mac (NVIDIA Parakeet)".to_string(),
        }
    }

    pub fn words(&self) -> usize {
        self.paragraphs
            .iter()
            .map(|p| p.split_whitespace().count())
            .sum()
    }
}

fn main() {
    if let Err(e) = run() {
        let msg = format!("{e:#}");
        eprintln!("{RED}✗{RESET} {msg}");
        report::record_error(&msg);
        eprintln!(
            "{MUTED}  If this looks like a bug, run transclip --report and paste it at {}{RESET}",
            report::NEW_ISSUE
        );
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    cleanup::install();

    if args.delete_model {
        let freed = transcribe::delete_model()?;
        if freed == 0 {
            eprintln!("{MUTED}•{RESET} There was no speech model to delete");
        } else {
            eprintln!(
                "{GREEN}✓{RESET} Deleted the speech model ({} MB freed)",
                text::thousands((freed / 1_000_000) as usize)
            );
        }
        return Ok(());
    }

    if args.report {
        let text = report::build();
        println!("{text}");
        if !args.no_copy {
            match clipboard::copy(&text) {
                Ok(()) => eprintln!(
                    "{GREEN}✓{RESET} Copied. Paste it into a new issue at {}",
                    report::NEW_ISSUE
                ),
                Err(e) => eprintln!(
                    "{AMBER}!{RESET} Couldn't copy it ({e:#}). Copy the text above into a new issue at {}",
                    report::NEW_ISSUE
                ),
            }
        }
        return Ok(());
    }

    let link = match &args.url {
        Some(u) => u.clone(),
        None => ask_for_link()?,
    };

    let sp = progress::spinner("Looking up video");
    let info = ytdlp::probe(&link);
    sp.finish_and_clear();
    let info = info?;
    if info.kind.as_deref() == Some("playlist") {
        bail!("That link is a playlist. Give me a link to a single video.");
    }
    match info.live_status.as_deref() {
        Some("is_live") => bail!("That's a live stream. Try again once it has finished."),
        Some("is_upcoming") => bail!("That stream hasn't started yet."),
        _ => {}
    }
    // Text from the internet is printed to the terminal, so strip anything
    // that could act as an escape code.
    let title = text::strip_controls(info.title.as_deref().unwrap_or("Untitled video"));
    let url = text::strip_controls(info.webpage_url.as_deref().unwrap_or(&link));
    let channel = info
        .channel
        .as_deref()
        .or(info.uploader.as_deref())
        .map(text::strip_controls);
    eprintln!("{GREEN}✓{RESET} {title}");
    // The Mac's speech model only understands English. Set when yt-dlp says
    // the video is in another language.
    let other_lang = info
        .language
        .clone()
        .filter(|l| l != "en" && !l.starts_with("en-"));

    // Everything downloaded goes in here and is deleted when we finish, or
    // by the cleanup handler if the run is interrupted.
    let tmp = tempfile::Builder::new().prefix("transclip-").tempdir()?;
    cleanup::register(tmp.path());

    let mut found: Option<(Source, Vec<Timed>)> = None;
    // After YouTube refuses captions for coming too often (HTTP 429), they're
    // left alone for a while and the audio is transcribed here instead. Not
    // for other languages: this Mac's model can't help, so captions are
    // still worth a try.
    let paused = if args.transcribe || other_lang.is_some() {
        None
    } else {
        cooldown::remaining()
    };
    let skip_captions = args.transcribe || paused.is_some();
    let mut refused = false;
    if args.transcribe {
        eprintln!("{MUTED}•{RESET} Skipping YouTube's captions and transcript, as asked");
    } else if let Some(mins) = paused {
        eprintln!(
            "{MUTED}•{RESET} YouTube recently said captions were asked for too often, so transcribing on this Mac (captions again in {mins} min)"
        );
    }
    if !skip_captions {
        match ytdlp::pick_caption_lang(&info, &args.lang) {
            Some(lang) => {
                let sp = progress::spinner(format!("Fetching captions ({lang})"));
                let got = ytdlp::download_captions(&url, &lang, tmp.path())
                    .and_then(|p| captions::load(&p));
                match got {
                    Ok(items) if !items.is_empty() => {
                        progress::done(&sp, format!("Using human-written captions ({lang})"));
                        found = Some((Source::Captions(lang), items));
                    }
                    Ok(_) => progress::warn(&sp, "The captions were empty"),
                    Err(e) if ytdlp::too_many_requests(&e) => {
                        refused = true;
                        cooldown::start();
                        progress::warn(&sp, refused_message(other_lang.is_some()));
                    }
                    Err(e) => progress::warn(&sp, format!("Couldn't use the captions ({e:#})")),
                }
            }
            None => eprintln!("{MUTED}•{RESET} No human-written captions"),
        }
    }

    // After a refusal the automatic transcript would be refused too, so only
    // ask for it then if it's the only hope (another language).
    if found.is_none() && !skip_captions && (!refused || other_lang.is_some()) {
        match ytdlp::pick_auto_caption_lang(&info, &args.lang) {
            Some(lang) => {
                let sp = progress::spinner("Fetching YouTube's automatic transcript");
                let got = ytdlp::download_auto_captions(&url, &lang, tmp.path())
                    .and_then(|p| captions::load(&p));
                match got {
                    Ok(items) if captions::is_punctuated(&items) => {
                        progress::done(&sp, "Using YouTube's automatic transcript");
                        found = Some((Source::AutoCaptions(lang), items));
                    }
                    // Unpunctuated, but still better than the English-only
                    // model guessing at another language.
                    Ok(items) if !items.is_empty() && other_lang.is_some() => {
                        progress::done(
                            &sp,
                            "Using YouTube's automatic transcript (no punctuation, but this Mac's model only does English)",
                        );
                        found = Some((Source::AutoCaptions(lang), items));
                    }
                    Ok(items) if !items.is_empty() => progress::warn(
                        &sp,
                        "YouTube's automatic transcript has no punctuation, transcribing on this Mac instead",
                    ),
                    Ok(_) => progress::warn(
                        &sp,
                        "YouTube's automatic transcript was empty, transcribing on this Mac instead",
                    ),
                    Err(e) if ytdlp::too_many_requests(&e) => {
                        refused = true;
                        cooldown::start();
                        progress::warn(&sp, refused_message(other_lang.is_some()));
                    }
                    Err(e) => progress::warn(
                        &sp,
                        format!(
                            "Couldn't use YouTube's automatic transcript ({e:#}), transcribing on this Mac instead"
                        ),
                    ),
                }
            }
            None => eprintln!(
                "{MUTED}•{RESET} No automatic transcript either, transcribing on this Mac"
            ),
        }
    }

    let (source, items) = match found {
        Some(f) => f,
        None => {
            if let Some(lang) = &other_lang {
                let hint = if args.transcribe {
                    "Leave off -t to use YouTube's own transcript."
                } else if refused {
                    "YouTube is refusing captions for now because they were asked for too often. Try again in about 30 minutes."
                } else {
                    "YouTube has no usable transcript for it either."
                };
                bail!(
                    "This video is in '{lang}', and the speech model on this Mac only understands English. {hint}"
                );
            }
            (Source::Parakeet, transcribe_audio(&url, tmp.path())?)
        }
    };

    let paragraphs = text::paragraphs(&items);
    if paragraphs.is_empty() {
        bail!("No speech found in that video.");
    }

    let t = Transcript {
        title,
        channel,
        url,
        duration: info.duration,
        source,
        paragraphs,
    };

    let copied = (!args.no_copy).then(|| clipboard::copy(&t.clipboard_text(args.text_only)));
    let interactive =
        !args.print && std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

    if interactive {
        viewer::run(&t, copied.as_ref().map(|r| r.is_ok()), args.text_only)?;
    } else {
        let mut out = std::io::stdout().lock();
        // Piping into something like `head` closes the pipe early; that's fine.
        if let Err(e) = writeln!(out, "{}", t.clipboard_text(args.text_only))
            && e.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(e.into());
        }
    }

    match copied {
        Some(Ok(())) => eprintln!(
            "{GREEN}✓{RESET} Copied to clipboard: {} words",
            text::thousands(t.words())
        ),
        Some(Err(e)) => eprintln!("{AMBER}!{RESET} Couldn't copy to the clipboard: {e:#}"),
        None => {}
    }
    Ok(())
}

/// Ask for the link at a prompt. Pasting here avoids the shell mangling
/// links that contain ? or &.
fn ask_for_link() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("Give me a video link");
    }
    eprint!("Paste the video link: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let link = line.trim().trim_matches(['"', '\'']).trim().to_string();
    if link.is_empty() {
        bail!("No link given.");
    }
    Ok(link)
}

/// What to say when YouTube refuses captions for coming too often.
fn refused_message(other_lang: bool) -> String {
    if other_lang {
        "YouTube says captions were asked for too often (HTTP 429)".to_string()
    } else {
        format!(
            "YouTube says captions were asked for too often (HTTP 429), so transcribing on this Mac and leaving captions alone for {} minutes",
            cooldown::PAUSE_SECS / 60
        )
    }
}

/// No usable captions: download the audio and transcribe it locally.
fn transcribe_audio(url: &str, dir: &Path) -> Result<Vec<Timed>> {
    let model_dir = transcribe::ensure_model()?;
    // YouTube sometimes refuses a download (HTTP 403) and allows the same
    // one a moment later, so try once more before giving up.
    let audio = match ytdlp::download_audio(url, dir) {
        Err(e) if format!("{e:#}").contains("HTTP Error 403") => {
            eprintln!("{MUTED}•{RESET} YouTube refused the download, trying once more");
            std::thread::sleep(std::time::Duration::from_secs(3));
            ytdlp::download_audio(url, dir)?
        }
        other => other?,
    };

    let sp = progress::spinner("Preparing audio");
    let wav = dir.join("audio-16k.wav");
    let converted = transcribe::to_wav16k(&audio, &wav);
    sp.finish_and_clear();
    converted?;
    let _ = std::fs::remove_file(&audio);
    let samples = transcribe::read_wav(&wav)?;
    let _ = std::fs::remove_file(&wav);

    let sp = progress::spinner("Loading speech model");
    let model = transcribe::load(&model_dir);
    sp.finish_and_clear();
    let mut model = model?;

    let bar = progress::bar("Transcribing");
    let started = Instant::now();
    let audio_secs = samples.len() as f64 / 16_000.0;
    let segs = transcribe::run(&mut model, &samples, |frac| {
        bar.set_position((frac * 1000.0) as u64);
        let elapsed = started.elapsed().as_secs_f64();
        if frac > 0.0 && frac < 1.0 {
            let left = elapsed / frac * (1.0 - frac);
            bar.set_prefix(format!("{} left", text::clock(left)));
        }
    });
    let segs = match segs {
        Ok(s) => s,
        Err(e) => {
            bar.finish_and_clear();
            return Err(e);
        }
    };
    progress::done(
        &bar,
        format!(
            "Transcribed {} of audio in {}",
            text::clock(audio_secs),
            text::clock(started.elapsed().as_secs_f64())
        ),
    );
    Ok(segs)
}
