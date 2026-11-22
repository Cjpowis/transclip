//! How a run says what it's doing: as lines in the terminal, with spinners
//! and bars (`--print`, or output going to a file or another program), or
//! by updating what the live screen draws.

use std::sync::{Mutex, MutexGuard, PoisonError};

use indicatif::ProgressBar;

use crate::progress::{self, AMBER, GREEN, MUTED, RESET};
use crate::viewer::Outcome;

/// The three places the words can come from, in the order they're tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Captions,
    Auto,
    Mac,
}

/// The stages of listening on this Mac.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Only the first time: the speech model isn't downloaded yet.
    Model,
    Audio,
    Prepare,
    Load,
    Transcribe,
    Copy,
}

impl Stage {
    /// What the live screen calls it: before, during and after.
    pub fn words(self) -> [&'static str; 3] {
        match self {
            Stage::Model => [
                "Get the speech model",
                "Getting speech model",
                "Got the speech model",
            ],
            Stage::Audio => ["Download audio", "Downloading audio", "Downloaded audio"],
            Stage::Prepare => ["Prepare audio", "Preparing audio", "Prepared audio"],
            Stage::Load => [
                "Load the speech model",
                "Loading speech model",
                "Loaded the speech model",
            ],
            Stage::Transcribe => ["Transcribe", "Transcribing", "Transcribed"],
            Stage::Copy => [
                "Copy to your clipboard",
                "Copying to clipboard",
                "Copied to your clipboard",
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceState {
    Waiting,
    Checking,
    Done(Outcome),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StageState {
    Waiting,
    /// `share` is how far through, when that's known.
    Working {
        share: Option<f64>,
        detail: String,
    },
    Done(String),
}

/// What the live screen shows while the work goes on.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub title: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub sources: [SourceState; 3],
    /// Empty until it starts listening on this Mac; then every stage at once.
    pub stages: Vec<(Stage, StageState)>,
    /// Warnings and notes the screen has no room for, to print when it
    /// closes, as they'd have been printed before.
    pub notes: Vec<String>,
}

impl Default for Live {
    fn default() -> Self {
        Live {
            title: None,
            channel: None,
            duration: None,
            sources: [
                SourceState::Waiting,
                SourceState::Waiting,
                SourceState::Waiting,
            ],
            stages: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// How a source went, as a line in the terminal.
pub enum Said {
    Done(String),
    Warn(String),
    Note(String),
    /// Nothing to print (it was said already, or will be).
    Quiet,
}

pub enum Show<'a> {
    /// Lines in the terminal, and the spinner or bar showing now.
    Lines(Mutex<Option<ProgressBar>>),
    Live(&'a Mutex<Live>),
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Show<'_> {
    pub fn lines() -> Show<'static> {
        Show::Lines(Mutex::new(None))
    }

    fn live(&self, f: impl FnOnce(&mut Live)) {
        if let Show::Live(state) = self {
            f(&mut lock(state));
        }
    }

    /// Lines only: show `pb` in place of any spinner or bar.
    fn put(&self, pb: ProgressBar) {
        if let Show::Lines(now) = self
            && let Some(old) = lock(now).replace(pb)
        {
            old.finish_and_clear();
        }
    }

    fn take(&self) -> Option<ProgressBar> {
        match self {
            Show::Lines(now) => lock(now).take(),
            Show::Live(_) => None,
        }
    }

    /// A short wait with only a spinner to show, like looking up the video.
    /// The live screen has a spinner of its own for this.
    pub fn busy(&self, msg: &str) {
        if matches!(self, Show::Lines(_)) {
            self.put(progress::spinner(msg.to_string()));
        }
    }

    /// Clear any spinner or bar, after an error or a step with no line.
    pub fn clear(&self) {
        if let Some(pb) = self.take() {
            pb.finish_and_clear();
        }
    }

    pub fn video(&self, title: &str, channel: Option<&str>, duration: Option<f64>) {
        match self {
            Show::Lines(_) => {
                self.clear();
                eprintln!("{GREEN}✓{RESET} {title}");
            }
            Show::Live(_) => self.live(|l| {
                l.title = Some(title.to_string());
                l.channel = channel.map(str::to_string);
                l.duration = duration;
            }),
        }
    }

    /// A line in the terminal, like "YouTube refused the download". The live
    /// screen keeps it for when it closes.
    pub fn note(&self, msg: &str) {
        match self {
            Show::Lines(_) => {
                self.clear();
                eprintln!("{MUTED}•{RESET} {msg}");
            }
            Show::Live(_) => self.live(|l| l.notes.push(format!("{MUTED}•{RESET} {msg}"))),
        }
    }

    pub fn checking(&self, which: Which, msg: &str) {
        self.busy(msg);
        self.live(|l| l.sources[which as usize] = SourceState::Checking);
    }

    pub fn checked(&self, which: Which, outcome: Outcome, said: Said) {
        if matches!(self, Show::Lines(_)) {
            let pb = self.take();
            match (said, pb) {
                (Said::Done(m), Some(pb)) => progress::done(&pb, m),
                (Said::Done(m), None) => eprintln!("{GREEN}✓{RESET} {m}"),
                (Said::Warn(m), Some(pb)) => progress::warn(&pb, m),
                (Said::Warn(m), None) => eprintln!("{AMBER}!{RESET} {m}"),
                (Said::Note(m), pb) => {
                    if let Some(pb) = pb {
                        pb.finish_and_clear();
                    }
                    eprintln!("{MUTED}•{RESET} {m}");
                }
                (Said::Quiet, pb) => {
                    if let Some(pb) = pb {
                        pb.finish_and_clear();
                    }
                }
            }
            return;
        }
        self.live(|l| {
            // The row says what happened; the reason is kept for later.
            if let Said::Warn(m) = &said {
                l.notes.push(format!("{AMBER}!{RESET} {m}"));
            }
            l.sources[which as usize] = SourceState::Done(outcome);
        });
    }

    /// Listening on this Mac is starting: every stage it will go through.
    pub fn stages(&self, list: &[Stage]) {
        self.live(|l| {
            l.sources[Which::Mac as usize] = SourceState::Checking;
            l.stages = list.iter().map(|s| (*s, StageState::Waiting)).collect();
        });
    }

    fn set(&self, stage: Stage, state: StageState) {
        self.live(|l| {
            if let Some(s) = l.stages.iter_mut().find(|(s, _)| *s == stage) {
                s.1 = state;
            }
        });
    }

    /// A stage starts. `msg` is its line in the terminal.
    pub fn start(&self, stage: Stage, msg: &str) {
        // Spinners and bars draw themselves, so they're only made for lines.
        if matches!(self, Show::Lines(_)) {
            match stage {
                Stage::Audio => {
                    let bar = progress::bar(msg.to_string());
                    // yt-dlp can take a little while before the first bytes.
                    bar.set_prefix("starting…");
                    self.put(bar);
                }
                Stage::Transcribe => self.put(progress::bar(msg.to_string())),
                Stage::Model => self.note(msg),
                Stage::Copy => {}
                Stage::Prepare | Stage::Load => self.busy(msg),
            }
        }
        self.set(
            stage,
            StageState::Working {
                share: None,
                detail: String::new(),
            },
        );
    }

    /// How far through a stage is, and a few words about it ("3.6/6.9 MB").
    pub fn progress(&self, stage: Stage, share: Option<f64>, detail: &str) {
        if let Show::Lines(now) = self
            && let Some(pb) = lock(now).as_ref()
        {
            if let Some(s) = share {
                pb.set_position((s * 1000.0) as u64);
            }
            if !detail.is_empty() {
                pb.set_prefix(detail.to_string());
            }
        }
        self.set(
            stage,
            StageState::Working {
                share,
                detail: detail.to_string(),
            },
        );
    }

    /// A stage is done. `line` is said in the terminal (or the spinner just
    /// goes); `detail` sits beside it on the live screen ("6.9 MB").
    pub fn finish(&self, stage: Stage, line: Option<String>, detail: &str) {
        match (line, self.take()) {
            (Some(m), Some(pb)) => progress::done(&pb, m),
            (Some(m), None) if matches!(self, Show::Lines(_)) => {
                eprintln!("{GREEN}✓{RESET} {m}")
            }
            (_, Some(pb)) => pb.finish_and_clear(),
            _ => {}
        }
        self.set(stage, StageState::Done(detail.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_screen_follows_the_work() {
        let state = Mutex::new(Live::default());
        let show = Show::Live(&state);
        show.video("A video", Some("A channel"), Some(60.0));
        show.checking(Which::Captions, "Fetching captions (en)");
        assert_eq!(lock(&state).sources[0], SourceState::Checking);
        show.checked(
            Which::Captions,
            Outcome::Nothing("none"),
            Said::Note("No human-written captions".into()),
        );
        show.stages(&[Stage::Audio, Stage::Transcribe]);
        show.start(Stage::Audio, "Downloading ");
        show.progress(Stage::Audio, Some(0.5), "3.4/6.9 MB");
        show.finish(Stage::Audio, Some("Downloaded audio".into()), "6.9 MB");
        let l = lock(&state).clone();
        assert_eq!(l.title.as_deref(), Some("A video"));
        assert_eq!(l.sources[0], SourceState::Done(Outcome::Nothing("none")));
        assert_eq!(l.sources[2], SourceState::Checking);
        assert_eq!(
            l.stages[0],
            (Stage::Audio, StageState::Done("6.9 MB".into()))
        );
        assert_eq!(l.stages[1], (Stage::Transcribe, StageState::Waiting));
    }

    #[test]
    fn warnings_and_notes_are_kept_for_when_the_screen_closes() {
        let state = Mutex::new(Live::default());
        let show = Show::Live(&state);
        show.note("Captions again in 30 min");
        show.checked(
            Which::Captions,
            Outcome::Nothing("couldn't fetch"),
            Said::Warn("Couldn't use the captions (HTTP 403)".into()),
        );
        // Said only to fill in the row, so not kept.
        show.checked(
            Which::Auto,
            Outcome::Nothing("none"),
            Said::Note("No automatic transcript either".into()),
        );
        let notes = lock(&state).notes.clone();
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].ends_with("Captions again in 30 min"));
        assert!(notes[1].ends_with("Couldn't use the captions (HTTP 403)"));
    }

    #[test]
    fn a_stage_that_is_not_listed_changes_nothing() {
        let state = Mutex::new(Live::default());
        let show = Show::Live(&state);
        show.stages(&[Stage::Audio]);
        show.finish(Stage::Copy, None, "");
        assert_eq!(
            lock(&state).stages,
            vec![(Stage::Audio, StageState::Waiting)]
        );
    }
}
