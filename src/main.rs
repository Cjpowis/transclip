mod captions;
mod cleanup;
mod clipboard;
mod cooldown;
mod locale;
mod progress;
mod report;
mod save;
mod show;
mod text;
mod transcribe;
mod viewer;
mod ytdlp;

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;

use crate::progress::{AMBER, GREEN, MUTED, RED, RESET};
use crate::show::{Said, Show, Stage, Which};
use crate::text::{Block, Timed};
use crate::viewer::Outcome;

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

    /// Caption language to look for first, like fr or de (default: this
    /// computer's language)
    #[arg(long, value_name = "CODE")]
    lang: Option<String>,

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

    /// Put the time each paragraph starts, like [12:40], in front of it
    #[arg(long)]
    timestamps: bool,

    /// Also save the transcript as a text file: in Downloads, or in the
    /// folder given (made if it's missing)
    #[arg(long, value_name = "FOLDER", num_args = 0..=1)]
    save: Option<Option<String>>,

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
    /// Chapter headings and paragraphs.
    pub blocks: Vec<Block>,
}

impl Transcript {
    /// The transcript itself: chapter headings, and paragraphs with the time
    /// they start in front when asked.
    pub fn body(&self, timestamps: bool) -> String {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Heading(h) => format!("## {h}"),
                Block::Para { start, text } if timestamps => {
                    format!("[{}] {text}", text::clock(f64::from(*start)))
                }
                Block::Para { text, .. } => text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// What's copied and saved: title and link on top unless asked not to.
    pub fn clipboard_text(&self, text_only: bool, timestamps: bool) -> String {
        if text_only {
            self.body(timestamps)
        } else {
            format!("{}\n{}\n\n{}", self.title, self.url, self.body(timestamps))
        }
    }

    /// Words spoken: headings and timestamps don't count.
    pub fn words(&self) -> usize {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Para { text, .. } => text.split_whitespace().count(),
                Block::Heading(_) => 0,
            })
            .sum()
    }
}

/// A plain answer rather than something going wrong: the link is a
/// playlist, the video's in another language, and so on. These aren't
/// followed by the line about reporting a bug.
#[derive(Debug)]
pub(crate) struct Plain(String);

impl std::fmt::Display for Plain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Plain {}

/// Stop with a plain answer (see `Plain`).
pub(crate) fn plain(msg: impl Into<String>) -> anyhow::Error {
    Plain(msg.into()).into()
}

fn main() {
    if let Err(e) = run() {
        let msg = format!("{e:#}");
        eprintln!("{RED}✗{RESET} {msg}");
        report::record_error(&msg);
        if e.downcast_ref::<Plain>().is_none() {
            eprintln!(
                "{MUTED}  If this looks like a bug, run transclip --report and paste it at {}{RESET}",
                report::NEW_ISSUE
            );
        }
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

    // `--save` takes an optional folder, so in `transclip --save <link>` the
    // link would be read as the folder. Spot that and treat it as the link.
    let mut url_arg = args.url.clone();
    let mut save_arg = args.save.clone();
    if url_arg.is_none()
        && let Some(Some(v)) = &save_arg
        && looks_like_link(v)
    {
        url_arg = Some(v.clone());
        save_arg = Some(None);
    }
    // Check (and make) the save folder before anything else happens.
    let save_dir = match &save_arg {
        Some(folder) => Some(save::prepare(folder.as_deref())?),
        None => None,
    };

    let link = match url_arg {
        Some(u) => u,
        None => ask_for_link()?,
    };
    // Sort out playlists now: YouTube takes several seconds to describe one.
    let link = match ytdlp::read_link(&link) {
        ytdlp::Link::Playlist => {
            return Err(plain(
                "That link is a playlist. Give me a link to a single video.",
            ));
        }
        ytdlp::Link::Mix(video) => {
            eprintln!(
                "{MUTED}•{RESET} That's a YouTube Mix, so transcribing the video it starts from"
            );
            format!("https://www.youtube.com/watch?v={video}")
        }
        ytdlp::Link::Other => link,
    };

    let interactive =
        !args.print && std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    let (got, saved_in_viewer) = if interactive {
        // The whole run happens on the live screen, which then shows the
        // transcript.
        let live = Mutex::new(show::Live::default());
        let show = Show::Live(&live);
        let done = viewer::run_live(
            &live,
            || fetch(&args, &link, save_dir.as_deref(), &show),
            |got| {
                got.copied = copy_transcript(&args, &got.t, &show);
                (
                    &got.t,
                    got.copied.as_ref().map(|r| r.is_ok()),
                    viewer::Options {
                        text_only: args.text_only,
                        timestamps: args.timestamps,
                        trail: got.trail,
                        transcribed_in: got.transcribed_in,
                        save_dir: save_dir.clone(),
                        saved: got
                            .saved
                            .as_ref()
                            .map(|r| r.as_ref().cloned().map_err(|e| format!("{e:#}"))),
                    },
                )
            },
        );
        // The warnings and notes the screen had no room for.
        let notes = std::mem::take(&mut live.lock().unwrap_or_else(PoisonError::into_inner).notes);
        let (got, saved_here) = match done {
            Ok(done) => done,
            Err(e) => {
                for note in &notes {
                    eprintln!("{note}");
                }
                return Err(e);
            }
        };
        summary(&got, &notes);
        (got, saved_here)
    } else {
        let show = Show::lines();
        let mut got = fetch(&args, &link, save_dir.as_deref(), &show)?;
        got.copied = copy_transcript(&args, &got.t, &show);
        let mut out = std::io::stdout().lock();
        // Piping into something like `head` closes the pipe early; that's fine.
        if let Err(e) = writeln!(
            out,
            "{}",
            got.t.clipboard_text(args.text_only, args.timestamps)
        ) && e.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(e.into());
        }
        (got, None)
    };

    match &got.copied {
        Some(Ok(())) => eprintln!(
            "{GREEN}✓{RESET} Copied to clipboard: {} words",
            text::thousands(got.t.words())
        ),
        Some(Err(e)) => eprintln!("{AMBER}!{RESET} Couldn't copy to the clipboard: {e:#}"),
        None => {}
    }
    if let Some(path) = &saved_in_viewer {
        eprintln!("{GREEN}✓{RESET} Saved to {}", save::display(path));
    }
    match got.saved {
        Some(Ok(path)) => eprintln!("{GREEN}✓{RESET} Saved to {}", save::display(&path)),
        // Saving with s after the automatic save failed puts that right.
        Some(Err(e)) if saved_in_viewer.is_none() => return Err(e),
        Some(Err(_)) | None => {}
    }
    Ok(())
}

/// A finished run: the transcript, where it came from, and how copying and
/// saving went.
struct Got {
    t: Transcript,
    trail: viewer::Trail,
    /// How many seconds transcribing on this Mac took, when it did.
    transcribed_in: Option<f64>,
    copied: Option<Result<()>>,
    saved: Option<Result<PathBuf>>,
}

/// Look the video up, get its words from the first place that has them,
/// then save the transcript (copying comes after). `show` says what's happening, as
/// lines or on the live screen.
fn fetch(args: &Args, link: &str, save_dir: Option<&Path>, show: &Show) -> Result<Got> {
    show.busy("Looking up video");
    let info = ytdlp::probe(link);
    show.clear();
    let (info, lookup) = info?;
    if info.kind.as_deref() == Some("playlist") {
        return Err(plain(
            "That link is a playlist. Give me a link to a single video.",
        ));
    }
    match info.live_status.as_deref() {
        Some("is_live") => {
            return Err(plain(
                "That's a live stream. Try again once it has finished.",
            ));
        }
        Some("is_upcoming") => return Err(plain("That stream hasn't started yet.")),
        _ => {}
    }
    // Text from the internet is printed to the terminal, so strip anything
    // that could act as an escape code.
    let title = text::strip_controls(info.title.as_deref().unwrap_or("Untitled video"));
    let url = text::strip_controls(info.webpage_url.as_deref().unwrap_or(link));
    let channel = info
        .channel
        .as_deref()
        .or(info.uploader.as_deref())
        .map(text::strip_controls);
    show.video(&title, channel.as_deref(), info.duration);
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

    // The downloads below reuse the lookup above rather than each asking
    // YouTube about the video again: quicker, and fewer requests.
    let lookup_file = tmp.path().join("video.info.json");
    let origin = match std::fs::write(&lookup_file, &lookup) {
        Ok(()) => ytdlp::Origin::Lookup(&lookup_file),
        Err(_) => ytdlp::Origin::Link(&url),
    };

    // The language to look for first: as asked, or this computer's own.
    let want = args.lang.clone().unwrap_or_else(locale::system_lang);
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
    // How each place the words can come from went, for the transcript screen.
    let mut trail = viewer::Trail {
        captions: Outcome::Nothing("none"),
        auto: Outcome::NotNeeded,
        mac: Outcome::NotNeeded,
    };
    if args.transcribe {
        show.note("Skipping YouTube's captions and transcript, as asked");
    } else if let Some(mins) = paused {
        show.note(&format!(
            "YouTube recently said captions were asked for too often, so transcribing on this Mac (captions again in {mins} min)"
        ));
    }
    if skip_captions {
        let skipped = Outcome::Nothing(if args.transcribe { "skipped" } else { "paused" });
        tell(&mut trail, show, Which::Captions, skipped, Said::Quiet);
        tell(&mut trail, show, Which::Auto, skipped, Said::Quiet);
    } else {
        match ytdlp::pick_caption_lang(&info, &want) {
            Some(lang) => {
                show.checking(Which::Captions, &format!("Fetching captions ({lang})"));
                let mut fetched = false;
                let got = ytdlp::download_captions(origin, &lang, tmp.path()).and_then(|p| {
                    fetched = true;
                    captions::load(&p)
                });
                match got {
                    Ok(items) if !items.is_empty() => {
                        let said = Said::Done(format!("Using human-written captions ({lang})"));
                        tell(&mut trail, show, Which::Captions, Outcome::Used, said);
                        found = Some((Source::Captions(lang), items));
                    }
                    Ok(_) => tell(
                        &mut trail,
                        show,
                        Which::Captions,
                        Outcome::Nothing("empty"),
                        Said::Warn("The captions were empty".into()),
                    ),
                    Err(e) if ytdlp::too_many_requests(&e) => {
                        refused = true;
                        cooldown::start();
                        let said = Said::Warn(refused_message(other_lang.is_some()));
                        tell(
                            &mut trail,
                            show,
                            Which::Captions,
                            Outcome::Nothing("refused"),
                            said,
                        );
                    }
                    Err(e) => {
                        let said = Said::Warn(format!("Couldn't use the captions ({e:#})"));
                        let outcome = Outcome::Nothing(failed(fetched));
                        tell(&mut trail, show, Which::Captions, outcome, said);
                    }
                }
            }
            None => tell(
                &mut trail,
                show,
                Which::Captions,
                Outcome::Nothing("none"),
                Said::Note("No human-written captions".into()),
            ),
        }
    }

    if found.is_none() && !skip_captions {
        // After a refusal the automatic transcript would be refused too, so
        // only ask for it then if it's the only hope (another language).
        if refused && other_lang.is_none() {
            let outcome = Outcome::Nothing("not tried");
            tell(&mut trail, show, Which::Auto, outcome, Said::Quiet);
        } else {
            auto_transcript(
                &info,
                &want,
                origin,
                tmp.path(),
                other_lang.is_some(),
                (&mut trail, show),
                (&mut found, &mut refused),
            );
        }
    }

    let mut transcribed_in = None;
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
                return Err(plain(format!(
                    "This video is in '{lang}', and the speech model on this Mac only understands English. {hint}"
                )));
            }
            // Every stage, shown before the first one starts.
            let mut stages = Vec::new();
            if transcribe::model_missing() {
                stages.push(Stage::Model);
            }
            stages.extend([Stage::Audio, Stage::Prepare, Stage::Load, Stage::Transcribe]);
            if !args.no_copy {
                stages.push(Stage::Copy);
            }
            show.stages(&stages);
            let (items, took) = transcribe_audio(origin, &url, tmp.path(), show)?;
            transcribed_in = Some(took);
            trail.mac = Outcome::Used;
            (Source::Parakeet, items)
        }
    };

    // The video's chapters become headings. Their titles come from the
    // internet, so they're cleaned like the video's title.
    let chapters: Vec<text::Chapter> = info
        .chapters
        .iter()
        .flatten()
        .filter_map(|c| {
            let start = c.start_time? as f32;
            let title = text::strip_controls(c.title.as_deref()?)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (!title.is_empty()).then_some(text::Chapter { start, title })
        })
        .collect();
    let blocks = text::blocks(&items, &chapters);
    if !blocks.iter().any(|b| matches!(b, Block::Para { .. })) {
        return Err(plain("No speech found in that video."));
    }

    let t = Transcript {
        title,
        channel,
        url,
        duration: info.duration,
        source,
        blocks,
    };

    // A saved file always has the title and link on top.
    let saved =
        save_dir.map(|dir| save::write(dir, &t.title, &t.clipboard_text(false, args.timestamps)));
    // Copying comes next (see `copy_transcript`); the live screen's last
    // frame shows it under way, which over SSH can take a moment.
    if !args.no_copy {
        show.start(Stage::Copy, "");
    }
    Ok(Got {
        t,
        trail,
        transcribed_in,
        copied: None,
        saved,
    })
}

/// Copy the transcript, unless that's turned off. Not on the worker: on the
/// live screen, copying over SSH writes to the terminal, and it mustn't land
/// in the middle of the screen being drawn.
fn copy_transcript(args: &Args, t: &Transcript, show: &Show) -> Option<Result<()>> {
    let copied = (!args.no_copy)
        .then(|| clipboard::copy(&t.clipboard_text(args.text_only, args.timestamps)));
    show.finish(Stage::Copy, None, "");
    copied
}

/// Try YouTube's automatic transcript. Sets `found` when it's used, and
/// `refused` when YouTube says it was asked for too often.
fn auto_transcript(
    info: &ytdlp::VideoInfo,
    want: &str,
    origin: ytdlp::Origin,
    dir: &Path,
    other_lang: bool,
    (trail, show): (&mut viewer::Trail, &Show),
    (found, refused): (&mut Option<(Source, Vec<Timed>)>, &mut bool),
) {
    let Some(lang) = ytdlp::pick_auto_caption_lang(info, want) else {
        let said = Said::Note("No automatic transcript either, transcribing on this Mac".into());
        tell(trail, show, Which::Auto, Outcome::Nothing("none"), said);
        return;
    };
    show.checking(Which::Auto, "Fetching YouTube's automatic transcript");
    let mut fetched = false;
    let got = ytdlp::download_auto_captions(origin, &lang, dir).and_then(|p| {
        fetched = true;
        captions::load(&p)
    });
    let (outcome, said) = match got {
        Ok(items) if captions::is_punctuated(&items) => {
            *found = Some((Source::AutoCaptions(lang), items));
            (
                Outcome::Used,
                Said::Done("Using YouTube's automatic transcript".into()),
            )
        }
        // Unpunctuated, but still better than the English-only model
        // guessing at another language.
        Ok(items) if !items.is_empty() && other_lang => {
            *found = Some((Source::AutoCaptions(lang), items));
            (
                Outcome::Used,
                Said::Done(
                    "Using YouTube's automatic transcript (no punctuation, but this Mac's model only does English)".into(),
                ),
            )
        }
        Ok(items) if !items.is_empty() => (
            Outcome::Nothing("no punctuation"),
            Said::Warn(
                "YouTube's automatic transcript has no punctuation, transcribing on this Mac instead"
                    .into(),
            ),
        ),
        Ok(_) => (
            Outcome::Nothing("empty"),
            Said::Warn(
                "YouTube's automatic transcript was empty, transcribing on this Mac instead".into(),
            ),
        ),
        Err(e) if ytdlp::too_many_requests(&e) => {
            *refused = true;
            cooldown::start();
            (
                Outcome::Nothing("refused"),
                Said::Warn(refused_message(other_lang)),
            )
        }
        Err(e) => (
            Outcome::Nothing(failed(fetched)),
            Said::Warn(format!(
                "Couldn't use YouTube's automatic transcript ({e:#}), transcribing on this Mac instead"
            )),
        ),
    };
    tell(trail, show, Which::Auto, outcome, said);
}

/// Note how one of the three places went: for the finished screen, and as
/// it happens.
fn tell(trail: &mut viewer::Trail, show: &Show, which: Which, outcome: Outcome, said: Said) {
    match which {
        Which::Captions => trail.captions = outcome,
        Which::Auto => trail.auto = outcome,
        Which::Mac => trail.mac = outcome,
    }
    show.checked(which, outcome, said);
}

/// After the live screen closes, what stays in the terminal: the title, any
/// warnings or notes, and where the words came from. The lines after it say
/// what was copied and saved.
fn summary(got: &Got, notes: &[String]) {
    let t = &got.t;
    eprintln!("{GREEN}✓{RESET} {}", t.title);
    for note in notes {
        eprintln!("{note}");
    }
    let from = match &t.source {
        Source::Captions(l) => format!(
            "Transcription taken from human-written captions ({})",
            l.trim_end_matches("-orig")
        ),
        Source::AutoCaptions(l) => format!(
            "Transcription taken from YouTube's automatic transcript ({})",
            l.trim_end_matches("-orig")
        ),
        Source::Parakeet => match (got.transcribed_in, t.duration) {
            (Some(took), Some(d)) => format!(
                "Created on this Mac: {} of audio in {}",
                text::clock(d),
                text::clock(took)
            ),
            (Some(took), None) => format!("Created on this Mac in {}", text::clock(took)),
            _ => "Created on this Mac".to_string(),
        },
    };
    eprintln!("{GREEN}✓{RESET} {from}");
}

/// Whether `--save`'s value is really the video link.
fn looks_like_link(s: &str) -> bool {
    s.contains("://") || s.starts_with("youtu") || s.starts_with("www.")
}

/// Ask for the link at a prompt. Pasting here avoids the shell mangling
/// links that contain ? or &.
fn ask_for_link() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return Err(plain("Give me a video link"));
    }
    eprint!("Paste the video link: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let link = line.trim().trim_matches(['"', '\'']).trim().to_string();
    if link.is_empty() {
        return Err(plain("No link given."));
    }
    Ok(link)
}

/// What the VIDEO box says when captions couldn't be used: they never
/// arrived, or they arrived but couldn't be read.
fn failed(fetched: bool) -> &'static str {
    if fetched {
        "unreadable"
    } else {
        "couldn't fetch"
    }
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
/// Returns the transcript and how many seconds transcribing took.
fn transcribe_audio(
    origin: ytdlp::Origin,
    url: &str,
    dir: &Path,
    show: &Show,
) -> Result<(Vec<Timed>, f64)> {
    let model_dir = transcribe::ensure_model(show)?;
    // YouTube sometimes refuses a download (HTTP 403) and allows the same
    // one a moment later, so try once more before giving up. The retry looks
    // the video up afresh, in case the saved lookup's links were the problem
    // (they expire after some hours, and the first model download is long).
    let audio = match ytdlp::download_audio(origin, dir, show) {
        Err(e) if format!("{e:#}").contains("HTTP Error 403") => {
            show.note("YouTube refused the download, trying once more");
            show.progress(Stage::Audio, None, "refused, trying again");
            std::thread::sleep(Duration::from_secs(3));
            ytdlp::download_audio(ytdlp::Origin::Link(url), dir, show)?
        }
        other => other?,
    };

    show.start(Stage::Prepare, "Preparing audio");
    let wav = dir.join("audio-16k.wav");
    let converted = transcribe::to_wav16k(&audio, &wav);
    show.finish(Stage::Prepare, None, "");
    converted?;
    let _ = std::fs::remove_file(&audio);
    let samples = transcribe::read_wav(&wav)?;
    let _ = std::fs::remove_file(&wav);

    show.start(Stage::Load, "Loading speech model");
    let model = transcribe::load(&model_dir);
    show.finish(Stage::Load, None, "");
    let mut model = model?;

    show.start(Stage::Transcribe, "Transcribing");
    let started = Instant::now();
    let audio_secs = samples.len() as f64 / 16_000.0;
    let glide = progress::Glide {
        piece: transcribe::SHORTEST_PIECE_SECS / audio_secs.max(1.0),
        guess_rate: GUESS_SPEED / audio_secs.max(1.0),
    };
    // The share done so far, and the seconds in when it was reported.
    let report = Mutex::new((0.0, 0.0));
    let finished = AtomicBool::new(false);
    let segs = std::thread::scope(|s| {
        // `run` only reports after each piece, so this moves the bar along in
        // between, at the speed measured so far.
        // If the system won't start another thread, the bar just jumps a
        // piece at a time, as it used to.
        let gliding = std::thread::Builder::new()
            .spawn_scoped(s, || {
                let mut shown: f64 = 0.0;
                while !finished.load(Ordering::Relaxed) {
                    let (done, at) = *report.lock().unwrap_or_else(PoisonError::into_inner);
                    let (now, left) = glide.at(done, at, started.elapsed().as_secs_f64());
                    shown = shown.max(now);
                    let left = left
                        .map(|l| format!("{} left", text::clock(l)))
                        .unwrap_or_default();
                    show.progress(Stage::Transcribe, Some(shown), &left);
                    std::thread::sleep(Duration::from_millis(100));
                }
            })
            .is_ok();
        let _stop = SetOnDrop(&finished);
        transcribe::run(&mut model, &samples, |frac| {
            *report.lock().unwrap_or_else(PoisonError::into_inner) =
                (frac, started.elapsed().as_secs_f64());
            if !gliding {
                show.progress(Stage::Transcribe, Some(frac), "");
            }
        })
    });
    let segs = match segs {
        Ok(s) => s,
        Err(e) => {
            show.clear();
            return Err(e);
        }
    };
    let took = started.elapsed().as_secs_f64();
    let (heard, spent) = (text::clock(audio_secs), text::clock(took));
    show.finish(
        Stage::Transcribe,
        Some(format!("Transcribed {heard} of audio in {spent}")),
        &format!("{heard} in {spent}"),
    );
    Ok((segs, took))
}

/// Seconds of audio transcribed per second, a cautious guess used only until
/// the first piece is done. An M2 Mac mini manages about 27.
const GUESS_SPEED: f64 = 15.0;

/// Sets the flag when dropped, so the bar's thread stops even if `run` panics.
struct SetOnDrop<'a>(&'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn plain_answers_are_told_apart_from_faults() {
        assert!(
            plain("That link is a playlist.")
                .downcast_ref::<Plain>()
                .is_some()
        );
        let wrapped: Result<()> = Err(plain("No link given.")).context("Starting");
        assert!(wrapped.unwrap_err().downcast_ref::<Plain>().is_some());
        let fault = anyhow::anyhow!("yt-dlp finished but no audio file was saved");
        assert!(fault.downcast_ref::<Plain>().is_none());
    }
}
