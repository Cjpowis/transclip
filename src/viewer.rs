//! The full screen: while the work goes on, a VIDEO box filling in as each
//! place the words can come from is tried; then the same box saying where
//! they came from, a scrollable TRANSCRIPT box, and the keys. c copies
//! again, s saves, q quits.

use std::io::stdout;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::show::{Live, SourceState, Stage, StageState};
use crate::{Source, Transcript, cleanup, clipboard, save, text};

const GREEN: Color = Color::Rgb(80, 200, 120);
const AMBER: Color = Color::Rgb(255, 184, 0);
const RED: Color = Color::Rgb(235, 90, 90);
const MUTED: Color = Color::Rgb(120, 140, 160);
/// Dark text on a coloured label.
const INK: Color = Color::Rgb(21, 24, 36);
/// The speech model on this Mac, as the VIDEO box names it.
const MODEL: &str = "NVIDIA Parakeet 0.6B, English";

/// How one of the places the words can come from went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The words came from here.
    Used,
    /// Not tried, because an earlier one worked.
    NotNeeded,
    /// Gave nothing, and why in a word or two ("none", "no punctuation").
    Nothing(&'static str),
}

/// The three places the words can come from, in the order they're tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trail {
    pub captions: Outcome,
    pub auto: Outcome,
    pub mac: Outcome,
}

pub struct Options {
    pub text_only: bool,
    pub timestamps: bool,
    pub trail: Trail,
    /// How many seconds transcribing on this Mac took, when it did.
    pub transcribed_in: Option<f64>,
    /// Where `s` saves: the `--save` folder, or Downloads when that's None.
    pub save_dir: Option<PathBuf>,
    /// What `--save` did before the viewer opened, if it was used.
    pub saved: Option<Result<PathBuf, String>>,
}

struct App<'a> {
    t: &'a Transcript,
    opts: Options,
    scroll: usize,
    wrapped_for: u16,
    /// Wrapped lines, and whether each is a (bold) chapter heading.
    lines: Vec<(String, bool)>,
    page: usize,
    status: (String, Color),
    /// Saved with `s` while the viewer was open.
    saved_here: Option<PathBuf>,
}

impl App<'_> {
    fn rewrap(&mut self, width: u16) {
        if width == self.wrapped_for {
            return;
        }
        self.wrapped_for = width;
        self.lines.clear();
        let w = width.max(10) as usize;
        for (i, b) in self.t.blocks.iter().enumerate() {
            if i > 0 {
                self.lines.push((String::new(), false));
            }
            let (s, heading) = match b {
                text::Block::Heading(h) => (h.clone(), true),
                text::Block::Para { start, text } if self.opts.timestamps => (
                    format!("[{}] {text}", text::clock(f64::from(*start))),
                    false,
                ),
                text::Block::Para { text, .. } => (text.clone(), false),
            };
            self.lines.extend(
                textwrap::wrap(&s, w)
                    .into_iter()
                    .map(|l| (l.into_owned(), heading)),
            );
        }
    }

    fn save(&mut self) {
        let already = match &self.opts.saved {
            Some(Ok(p)) => Some(p.clone()),
            _ => self.saved_here.clone(),
        };
        if let Some(p) = already {
            self.status = (format!("✓ Already saved to {}", save::display(&p)), GREEN);
            return;
        }
        let dir = self.opts.save_dir.clone().unwrap_or_else(save::default_dir);
        let text = self.t.clipboard_text(false, self.opts.timestamps);
        self.status =
            match save::ready(dir, false).and_then(|d| save::write(&d, &self.t.title, &text)) {
                Ok(p) => {
                    let s = (format!("✓ Saved to {}", save::display(&p)), GREEN);
                    self.saved_here = Some(p);
                    s
                }
                Err(e) => (format!("✗ Couldn't save: {e:#}"), RED),
            };
    }

    fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.page.max(1))
    }

    fn scroll_by(&mut self, delta: isize) {
        let next = self.scroll as isize + delta;
        self.scroll = next.clamp(0, self.max_scroll() as isize) as usize;
    }

    fn copy(&mut self) {
        let text = self
            .t
            .clipboard_text(self.opts.text_only, self.opts.timestamps);
        let chars = text.chars().count();
        self.status = match clipboard::copy(&text) {
            Ok(()) => (
                format!("✓ Copied again ({} characters)", text::thousands(chars)),
                GREEN,
            ),
            Err(e) => (format!("✗ Couldn't copy: {e}"), RED),
        };
    }
}

/// Do `work` with the live screen showing how it's going, then show what it
/// made on the same screen. `view` runs here once the work is done (so it
/// can copy without landing in the middle of drawing) and picks out the
/// transcript, whether it was copied (None if copying was turned off) and
/// the screen's options. Returns what `work` made, and where the transcript
/// was saved with `s`, if it was. q or Ctrl-C while it works stops the run,
/// as Ctrl-C does.
pub fn run_live<T: Send>(
    live: &Mutex<Live>,
    work: impl FnOnce() -> Result<T> + Send,
    view: impl FnOnce(&mut T) -> (&Transcript, Option<bool>, Options),
) -> Result<(T, Option<PathBuf>)> {
    let mut terminal = cleanup::on_screen(|| {
        cleanup::screen_up(true);
        let terminal = ratatui::init();
        let _ = execute!(stdout(), EnableMouseCapture);
        terminal
    });
    let close = || {
        cleanup::on_screen(|| {
            cleanup::screen_up(false);
            let _ = execute!(stdout(), DisableMouseCapture);
            ratatui::restore();
        })
    };
    let done = std::thread::scope(|s| {
        let worker = std::thread::Builder::new()
            .spawn_scoped(s, work)
            .map_err(|e| anyhow!("couldn't start working: {e}"))?;
        let started = Instant::now();
        let frame = |terminal: &mut DefaultTerminal| {
            let now = live.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let tick = (started.elapsed().as_millis() / 100) as usize;
            let _ = cleanup::on_screen(|| terminal.draw(|f| draw_working(f, &now, tick)));
        };
        while !worker.is_finished() {
            frame(&mut terminal);
            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(Event::Key(k)) = event::read()
                && k.kind == KeyEventKind::Press
                && (matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (k.modifiers.contains(KeyModifiers::CONTROL)
                        && k.code == KeyCode::Char('c')))
            {
                // The work reports each step here, so holding this makes it
                // wait at its next step rather than start something new.
                let _hold = live.lock().unwrap_or_else(PoisonError::into_inner);
                close();
                eprint!("Stopped.");
                cleanup::stop();
            }
        }
        // Once more, for how the work left it (copying under way).
        frame(&mut terminal);
        worker
            .join()
            .map_err(|_| anyhow!("Transclip stopped unexpectedly"))?
    });
    let mut got = match done {
        Ok(got) => got,
        Err(e) => {
            close();
            return Err(e);
        }
    };
    let (t, copied, opts) = view(&mut got);
    let result = finished(&mut terminal, t, copied, opts);
    close();
    Ok((got, result?))
}

/// The finished screen, until q. `copied`: None if auto-copy was turned
/// off, otherwise whether it worked. Returns where the transcript was saved
/// with `s`, if it was.
fn finished(
    terminal: &mut DefaultTerminal,
    t: &Transcript,
    copied: Option<bool>,
    opts: Options,
) -> Result<Option<PathBuf>> {
    let mut status = match copied {
        Some(true) => ("✓ Copied to your clipboard".to_string(), GREEN),
        Some(false) => ("! Couldn't copy. Press c to try again".to_string(), AMBER),
        None => ("Press c to copy".to_string(), MUTED),
    };
    match &opts.saved {
        Some(Ok(p)) => status.0 = format!("{}, saved to {}", status.0, save::display(p)),
        Some(Err(e)) => status = (format!("✗ Couldn't save: {e}"), RED),
        None => {}
    }
    let mut app = App {
        t,
        opts,
        scroll: 0,
        wrapped_for: 0,
        lines: Vec::new(),
        page: 1,
        status,
        saved_here: None,
    };
    event_loop(terminal, &mut app).map(|()| app.saved_here)
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        cleanup::on_screen(|| terminal.draw(|f| draw(f, app)))?;
        if !event::poll(Duration::from_millis(500))? {
            continue;
        }
        let page = app.page.saturating_sub(2).max(1) as isize;
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                    return Ok(());
                }
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') => app.copy(),
                    KeyCode::Char('s') => app.save(),
                    KeyCode::Down | KeyCode::Char('j') => app.scroll_by(1),
                    KeyCode::Up | KeyCode::Char('k') => app.scroll_by(-1),
                    KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('f') => {
                        app.scroll_by(page)
                    }
                    KeyCode::PageUp | KeyCode::Char('b') => app.scroll_by(-page),
                    KeyCode::Home | KeyCode::Char('g') => app.scroll = 0,
                    KeyCode::End | KeyCode::Char('G') => app.scroll = app.max_scroll(),
                    _ => {}
                }
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::ScrollDown => app.scroll_by(3),
                MouseEventKind::ScrollUp => app.scroll_by(-3),
                _ => {}
            },
            _ => {}
        }
    }
}

fn label_style() -> Style {
    Style::new().fg(INK).bg(GREEN).add_modifier(Modifier::BOLD)
}

/// A rounded box with a highlighted label, and maybe a note, on its top edge.
fn boxed(label: &str, note: Option<String>) -> Block<'static> {
    let mut title = vec![
        Span::styled("─", Style::new().fg(MUTED)),
        Span::styled(format!(" {label} "), label_style()),
    ];
    if let Some(note) = note {
        title.push(Span::styled(format!(" {note} "), Style::new().fg(MUTED)));
    }
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(MUTED))
        .title(Line::from(title))
        .padding(Padding::horizontal(1))
}

/// One of the three places: number, mark, name, and how it went on the
/// right. On a narrow screen how it went moves to its own line underneath;
/// `keep` is the room it keeps for how it went (see `spread`).
fn source_row(
    n: u8,
    name: &str,
    outcome: Outcome,
    width: usize,
    keep: usize,
) -> Vec<Line<'static>> {
    let (mark, right, style) = match outcome {
        Outcome::Used => (
            "✓",
            Span::styled(" USED ", label_style()),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Outcome::NotNeeded => ("–", Span::raw("not needed"), Style::new().fg(MUTED)),
        Outcome::Nothing(why) => ("✗", Span::raw(why), Style::new().fg(MUTED)),
    };
    spread(format!("{n} {mark} {name}"), right, style, width, keep)
}

/// The channel and the video's length, under the title. A long channel
/// name would push the length off the end, so then they get a line each.
fn details_lines(channel: Option<&str>, duration: Option<f64>, width: usize) -> Vec<Line<'static>> {
    let mut details: Vec<String> = channel.map(str::to_string).into_iter().collect();
    details.extend(duration.map(text::clock));
    let joined = details.join(" · ");
    if !details.is_empty() && textwrap::core::display_width(&joined) <= width {
        details = vec![joined];
    }
    details
        .iter()
        .flat_map(|d| textwrap::wrap(d, width))
        .map(|l| Line::from(l.into_owned()).fg(MUTED))
        .collect()
}

/// `s` cut to fit `width` columns, ending "…" if anything was cut.
fn cut(s: &str, width: usize) -> String {
    let w = textwrap::core::display_width;
    if w(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if w(&out) + w(ch.encode_utf8(&mut [0; 4])) + 1 > width {
            break;
        }
        out.push(ch);
    }
    out.trim_end().to_string() + "…"
}

/// The most room a source row's result needs while the work goes on
/// ("no punctuation", "couldn't fetch"), so a row that has to split on a
/// narrow screen does so from the start, and the box keeps its size.
const RESULT_ROOM: usize = 14;

/// `left` at the left and `right` at the right edge, or on the line
/// underneath when they won't fit side by side, keeping at least `keep`
/// columns for `right`.
fn spread(
    left: String,
    right: Span<'static>,
    style: Style,
    width: usize,
    keep: usize,
) -> Vec<Line<'static>> {
    let left = Span::raw(left);
    let room = width.saturating_sub(left.width() + right.width().max(keep));
    if room >= 1 {
        let room = width.saturating_sub(left.width() + right.width());
        return vec![Line::from(vec![left, Span::raw(" ".repeat(room)), right]).style(style)];
    }
    // The name wraps too, so nothing on the end of it (the language) is cut
    // off on a very narrow screen.
    let wrap = textwrap::Options::new(width.max(8)).subsequent_indent("    ");
    let mut out: Vec<Line<'static>> = textwrap::wrap(&left.content, wrap)
        .into_iter()
        .map(|l| Line::from(l.into_owned()).style(style))
        .collect();
    let pad = width.saturating_sub(right.width()).max(4);
    out.push(Line::from(vec![Span::raw(" ".repeat(pad)), right]).style(style));
    out
}

/// Not reached yet, on the live screen.
const DIM: Color = Color::Rgb(95, 107, 125);
/// The empty part of a bar.
const BAR_REST: Color = Color::Rgb(61, 90, 138);
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How a stage that's working is getting on, from the most said to the
/// least, so the most that fits can be shown on one line.
fn working_details(share: Option<f64>, detail: &str) -> Vec<Vec<Span<'static>>> {
    let short = detail.split(" · ").next().unwrap_or("");
    let words = |d: &str| {
        if d.is_empty() {
            vec![]
        } else {
            vec![Span::raw(format!(" {d}"))]
        }
    };
    let Some(share) = share else {
        return vec![words(detail), words(short), vec![]];
    };
    let n = (share.clamp(0.0, 1.0) * 10.0).round() as usize;
    let bar = vec![
        Span::styled("█".repeat(n), Style::new().fg(GREEN)),
        Span::styled("░".repeat(10 - n), Style::new().fg(BAR_REST)),
    ];
    let pct = Span::raw(format!("{:>4}%", (share * 100.0).round() as u32));
    let with = |extra: Vec<Span<'static>>| {
        let mut v = bar.clone();
        v.push(pct.clone());
        v.extend(extra);
        v
    };
    vec![
        with(words(detail)),
        with(words(short)),
        with(vec![]),
        vec![pct.clone()],
        vec![],
    ]
}

/// One stage of listening on this Mac. Always one line, so the box doesn't
/// change size as the stages go by: what's said on the right shrinks to fit.
fn stage_row(stage: Stage, state: &StageState, width: usize) -> Line<'static> {
    let [todo, doing, did] = stage.words();
    let (mark, label, style, choices) = match state {
        StageState::Waiting => ("○", todo, Style::new().fg(DIM), vec![vec![]]),
        StageState::Working { share, detail } => (
            "▸",
            doing,
            Style::new().add_modifier(Modifier::BOLD),
            working_details(*share, detail),
        ),
        StageState::Done(d) if d.is_empty() => ("✓", did, Style::new(), vec![vec![]]),
        StageState::Done(d) => (
            "✓",
            did,
            Style::new(),
            vec![
                vec![Span::styled(d.clone(), Style::new().fg(MUTED))],
                vec![],
            ],
        ),
    };
    let left = Span::raw(format!("    {mark} {label}"));
    let room = width.saturating_sub(left.width() + 1);
    let right = choices
        .into_iter()
        .find(|c| c.iter().map(Span::width).sum::<usize>() <= room)
        .unwrap_or_default();
    let pad = width.saturating_sub(left.width() + right.iter().map(Span::width).sum::<usize>());
    let mut spans = vec![left, Span::raw(" ".repeat(pad))];
    spans.extend(right);
    Line::from(spans).style(style)
}

/// The VIDEO box while the work goes on. One spinner at a time: on the title
/// while the video is looked up, then on the place being tried. On "Listening
/// on this Mac" it stays put while its stages tick by, the one being worked
/// on marked ▸. `compact` leaves out the extras on a short screen.
fn working_lines(live: &Live, width: usize, tick: usize, compact: bool) -> Vec<Line<'static>> {
    let spin = SPINNER[tick % SPINNER.len()];
    // The title and the channel get a line each, cut short if need be (the
    // finished screen shows them whole), so the box doesn't change size when
    // they arrive.
    let title = match &live.title {
        Some(title) => cut(title, width),
        None => format!("{spin} Looking up the video…"),
    };
    let mut out: Vec<Line<'static>> = vec![Line::from(title).bold()];
    if !compact {
        let length = live.duration.map(text::clock);
        let details = match (&live.channel, length) {
            (Some(c), Some(l)) => {
                let room = width.saturating_sub(textwrap::core::display_width(&l) + 3);
                format!("{} · {l}", cut(c, room))
            }
            (Some(c), None) => cut(c, width),
            (None, Some(l)) => l,
            (None, None) => String::new(),
        };
        out.push(Line::from(details).fg(MUTED));
        out.push(Line::default());
    }
    out.push(Line::from(Span::styled(
        " CREATING THE TRANSCRIPT ",
        label_style(),
    )));

    let total = live.stages.len();
    let step = live
        .stages
        .iter()
        .position(|(_, s)| !matches!(s, StageState::Done(_)))
        .map_or(total, |i| i + 1);
    let names = [
        "Human-written captions",
        "YouTube's automatic transcript",
        "Listening on this Mac",
    ];
    for (i, (name, state)) in names.iter().zip(&live.sources).enumerate() {
        let n = i as u8 + 1;
        out.extend(match state {
            SourceState::Done(outcome) => source_row(n, name, *outcome, width, RESULT_ROOM),
            SourceState::Waiting => spread(
                format!("{n} ○ {name}"),
                Span::raw(if i < 2 { "waiting" } else { "if needed" }),
                Style::new().fg(DIM),
                width,
                RESULT_ROOM,
            ),
            SourceState::Checking => {
                let right = if i == 2 && total > 0 {
                    format!("step {step} of {total}")
                } else {
                    "checking…".to_string()
                };
                spread(
                    format!("{n} {spin} {name}"),
                    Span::raw(right),
                    Style::new().add_modifier(Modifier::BOLD),
                    width,
                    RESULT_ROOM,
                )
            }
        });
    }
    if !live.stages.is_empty() {
        if !compact {
            out.push(Line::from(format!("    Model: {MODEL}")).fg(MUTED));
        }
        out.extend(live.stages.iter().map(|(s, st)| stage_row(*s, st, width)));
    }
    out
}

/// Wider than this, the screen stays this wide in the middle, so lines of
/// transcript don't run right across a big desktop window.
const MAX_WIDTH: u16 = 100;

fn capped(full: Rect) -> Rect {
    if full.width > MAX_WIDTH {
        Rect {
            x: full.x + (full.width - MAX_WIDTH) / 2,
            width: MAX_WIDTH,
            ..full
        }
    } else {
        full
    }
}

/// The live screen: the VIDEO box filling in, an empty TRANSCRIPT box, and
/// the one key that works.
fn draw_working(f: &mut Frame, live: &Live, tick: usize) {
    let area = capped(f.area());
    let width = area.width.saturating_sub(4).max(10) as usize;
    let mut video = working_lines(live, width, tick, false);
    let compact = video.len() as u16 + 2 + 1 + 3 + 1 > area.height;
    if compact {
        video = working_lines(live, width, tick, true);
    }
    let [video_area, _, body_area, keys_area] = Layout::vertical([
        Constraint::Length(video.len() as u16 + 2),
        Constraint::Length(if compact { 0 } else { 1 }),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);
    f.render_widget(
        Paragraph::new(video).block(boxed("VIDEO", None)),
        video_area,
    );

    let body = boxed("TRANSCRIPT", Some("working…".to_string()));
    let inner = body.inner(body_area);
    let mut lines = vec![Line::default(); (inner.height / 2).saturating_sub(1) as usize];
    lines.extend(
        textwrap::wrap(
            "The transcript appears here when it's ready.",
            inner.width.max(1) as usize,
        )
        .into_iter()
        .map(|l| Line::from(l.into_owned()).fg(MUTED).centered()),
    );
    f.render_widget(Paragraph::new(lines).block(body), body_area);

    let key = Style::new().fg(INK).bg(MUTED).add_modifier(Modifier::BOLD);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" q ", key),
            Span::raw(" Stop"),
        ])),
        keys_area,
    );
}

/// How much the VIDEO box shows, so a short screen still has room to read.
#[derive(Clone, Copy, PartialEq)]
enum Fit {
    /// Everything.
    Full,
    /// Just the title, the heading, the source that was used and the status.
    Short,
    /// As Short, without the heading.
    Tiny,
}

/// The VIDEO box's lines.
fn video_lines(app: &App, width: usize, fit: Fit) -> Vec<Line<'static>> {
    let compact = fit != Fit::Full;
    let t = app.t;
    let trail = app.opts.trail;
    let mut out: Vec<Line<'static>> = textwrap::wrap(&t.title, width)
        .into_iter()
        .take(if compact { 1 } else { 2 })
        .map(|l| Line::from(l.into_owned()).bold())
        .collect();
    if !compact {
        out.extend(details_lines(t.channel.as_deref(), t.duration, width));
        out.push(Line::default());
    }

    let heading = if trail.mac == Outcome::Used {
        "CREATED USING"
    } else {
        "TRANSCRIPTION TAKEN FROM"
    };
    if fit != Fit::Tiny {
        out.push(Line::from(Span::styled(
            format!(" {heading} "),
            label_style(),
        )));
    }
    // YouTube marks the speaker's own automatic transcript "en-orig"; just
    // the language is clearer.
    let lang = match &t.source {
        Source::Captions(l) | Source::AutoCaptions(l) => Some(l.trim_end_matches("-orig")),
        Source::Parakeet => None,
    };
    let rows = [
        (1, "Human-written captions", trail.captions),
        (2, "YouTube's automatic transcript", trail.auto),
        (3, "Listening on this Mac", trail.mac),
    ];
    for (n, name, outcome) in rows {
        if compact && outcome != Outcome::Used {
            continue;
        }
        let name = match (outcome, lang) {
            (Outcome::Used, Some(l)) if n < 3 => format!("{name} ({l})"),
            _ => name.to_string(),
        };
        out.extend(source_row(n, &name, outcome, width, 0));
    }
    if trail.mac == Outcome::Used && !compact {
        out.push(Line::from(format!("    Model: {MODEL}")).fg(MUTED));
        if let Some(took) = app.opts.transcribed_in {
            let line = match t.duration {
                Some(d) => format!(
                    "    Transcribed {} of audio in {}",
                    text::clock(d),
                    text::clock(took)
                ),
                None => format!("    Transcribed in {}", text::clock(took)),
            };
            out.push(Line::from(line).fg(MUTED));
        }
    }
    if !compact {
        out.push(Line::default());
    }

    // The status can be long (a saved file's path), so it wraps.
    out.extend(
        textwrap::wrap(&app.status.0, width)
            .into_iter()
            .map(|l| Line::from(l.into_owned()).fg(app.status.1)),
    );
    out
}

/// The keys, with smaller gaps on a narrow screen, then without Scroll,
/// then just Quit, so Quit is never cut off.
fn keys_line(width: usize) -> Line<'static> {
    let key = Style::new().fg(INK).bg(MUTED).add_modifier(Modifier::BOLD);
    let build = |gap: usize, scroll: bool| {
        let mut spans = Vec::new();
        for (k, what) in [
            ("c", "Copy"),
            ("s", "Save"),
            ("↑↓", "Scroll"),
            ("q", "Quit"),
        ] {
            if what == "Scroll" && !scroll {
                continue;
            }
            spans.push(Span::styled(format!(" {k} "), key));
            spans.push(Span::raw(format!(" {what}{}", " ".repeat(gap))));
        }
        Line::from(spans)
    };
    let roomy = build(3, true);
    if roomy.width() <= width {
        return roomy;
    }
    let tight = build(1, true);
    if tight.width() <= width {
        return tight;
    }
    let fewer = build(1, false);
    if fewer.width() <= width {
        return fewer;
    }
    Line::from(vec![Span::styled(" q ", key), Span::raw(" Quit")])
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = capped(f.area());
    // Each box has a border and a column of padding on each side.
    let width = area.width.saturating_sub(4).max(10) as usize;
    // Keep at least four lines of transcript in view (six rows with the
    // TRANSCRIPT box's edges), plus the keys, and a gap when there's room.
    let mut fit = Fit::Full;
    let mut video = video_lines(app, width, fit);
    if video.len() as u16 + 2 + 1 + 6 + 1 > area.height {
        fit = Fit::Short;
        video = video_lines(app, width, fit);
    }
    if fit == Fit::Short && video.len() as u16 + 2 + 6 + 1 > area.height {
        fit = Fit::Tiny;
        video = video_lines(app, width, fit);
    }
    let compact = fit != Fit::Full;
    let [video_area, _, body_area, keys_area] = Layout::vertical([
        Constraint::Length(video.len() as u16 + 2),
        Constraint::Length(if compact { 0 } else { 1 }),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    f.render_widget(
        Paragraph::new(video).block(boxed("VIDEO", None)),
        video_area,
    );

    let words = format!("{} words", text::thousands(app.t.words()));
    let body = boxed("TRANSCRIPT", Some(words));
    let inner = body.inner(body_area);
    app.rewrap(inner.width);
    app.page = inner.height as usize;
    app.scroll = app.scroll.min(app.max_scroll());
    let pct = if app.max_scroll() == 0 {
        100
    } else {
        app.scroll * 100 / app.max_scroll()
    };
    let visible: Vec<Line> = app
        .lines
        .iter()
        .skip(app.scroll)
        .take(inner.height as usize)
        .map(|(l, heading)| {
            if *heading {
                Line::from(l.as_str()).bold()
            } else {
                Line::from(l.as_str())
            }
        })
        .collect();
    let body = body.title_bottom(Line::from(format!(" {pct}% ")).right_aligned().fg(MUTED));
    f.render_widget(Paragraph::new(visible).block(body), body_area);

    f.render_widget(
        Paragraph::new(keys_line(keys_area.width as usize)),
        keys_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn transcript(source: Source) -> Transcript {
        Transcript {
            title: "How bridges hold themselves up".into(),
            channel: Some("Channel name".into()),
            url: "https://youtu.be/x".into(),
            duration: Some(900.0),
            source,
            blocks: vec![text::Block::Para {
                start: 0.0,
                text: "A bridge is always doing two jobs at once.".into(),
            }],
        }
    }

    /// The screen as text, `w` columns by `h` rows.
    fn screen(t: &Transcript, trail: Trail, took: Option<f64>, w: u16, h: u16) -> String {
        let opts = Options {
            text_only: false,
            timestamps: false,
            trail,
            transcribed_in: took,
            save_dir: None,
            saved: None,
        };
        let mut app = App {
            t,
            opts,
            scroll: 0,
            wrapped_for: 0,
            lines: Vec::new(),
            page: 1,
            status: ("✓ Copied to your clipboard".into(), GREEN),
            saved_here: None,
        };
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        as_text(&term)
    }

    /// The live screen as text, `w` columns by `h` rows.
    fn working_screen(live: &Live, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_working(f, live, 0)).unwrap();
        as_text(&term)
    }

    fn as_text(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let a = buf.area;
        (0..a.height)
            .map(|y| {
                (0..a.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// How many rows the VIDEO box takes, edges included.
    fn video_box_rows(s: &str) -> usize {
        s.lines()
            .position(|r| r.trim_start().starts_with('╰'))
            .unwrap()
            + 1
    }

    /// Made on this Mac: two places tried, three stages done, loading.
    fn listening() -> Live {
        Live {
            title: Some("How bridges hold themselves up".into()),
            channel: Some("Channel name".into()),
            duration: Some(900.0),
            sources: [
                SourceState::Done(Outcome::Nothing("none")),
                SourceState::Done(Outcome::Nothing("no punctuation")),
                SourceState::Checking,
            ],
            stages: vec![
                (Stage::Audio, StageState::Done("12 MB".into())),
                (Stage::Prepare, StageState::Done(String::new())),
                (
                    Stage::Load,
                    StageState::Working {
                        share: None,
                        detail: String::new(),
                    },
                ),
                (Stage::Transcribe, StageState::Waiting),
                (Stage::Copy, StageState::Waiting),
            ],
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_live_screen_starts_with_every_place_waiting() {
        let s = working_screen(&Live::default(), 59, 40);
        for want in [
            "VIDEO",
            "Looking up the video…",
            "CREATING THE TRANSCRIPT",
            "1 ○ Human-written captions",
            "waiting",
            "3 ○ Listening on this Mac",
            "if needed",
            "working…",
            "The transcript appears here when it's ready.",
            "q  Stop",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        assert!(!s.contains("Model:"), "{s}");
    }

    #[test]
    fn checking_youtube_keeps_the_box_the_same_size() {
        let mut live = Live {
            title: Some("How bridges hold themselves up".into()),
            channel: Some("Channel name".into()),
            duration: Some(900.0),
            ..Live::default()
        };
        let looking_up = video_box_rows(&working_screen(&Live::default(), 59, 40));
        let before = video_box_rows(&working_screen(&live, 59, 40));
        assert_eq!(before, looking_up);
        live.sources[0] = SourceState::Done(Outcome::Nothing("none"));
        live.sources[1] = SourceState::Checking;
        let s = working_screen(&live, 59, 40);
        for want in [
            "1 ✗ Human-written captions",
            "none",
            "2 ⠋ YouTube's automatic transcript",
            "checking…",
            "Channel name · 15:00",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        assert_eq!(video_box_rows(&s), before, "{s}");
    }

    #[test]
    fn listening_on_this_mac_shows_every_stage_and_one_spinner() {
        let s = working_screen(&listening(), 59, 40);
        for want in [
            "3 ⠋ Listening on this Mac",
            "step 3 of 5",
            "Model: NVIDIA Parakeet 0.6B, English",
            "✓ Downloaded audio",
            "12 MB",
            "✓ Prepared audio",
            "▸ Loading speech model",
            "○ Transcribe",
            "○ Copy to your clipboard",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        assert_eq!(s.matches('⠋').count(), 1, "{s}");
    }

    #[test]
    fn a_narrow_phone_keeps_each_stage_on_one_line() {
        let mut live = listening();
        live.stages[2].1 = StageState::Done(String::new());
        live.stages[3].1 = StageState::Working {
            share: Some(0.4),
            detail: "0:13 left · 3:20 of 15:00".into(),
        };
        let wide = video_box_rows(&working_screen(&live, 59, 40));
        let s = working_screen(&live, 45, 40);
        for want in ["▸ Transcribing", "40%", "step 4 of 5", "ready.", "q  Stop"] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        // Only the sources whose result moved underneath add rows.
        let narrow = video_box_rows(&s);
        assert!(narrow <= wide + 3, "{s}");
        let row = s.lines().find(|r| r.contains("Transcribing")).unwrap();
        assert!(row.contains("40%"), "{row:?}");
    }

    #[test]
    fn on_narrow_phones_the_box_keeps_its_size_until_listening() {
        for w in [43, 45] {
            let mut live = Live::default();
            let size = video_box_rows(&working_screen(&live, w, 40));
            live.title = Some("A title long enough to need two lines on a phone".into());
            live.channel = Some("A channel name that is very long indeed".into());
            live.duration = Some(900.0);
            live.sources[0] = SourceState::Checking;
            let s = working_screen(&live, w, 40);
            assert_eq!(video_box_rows(&s), size, "{s}");
            assert!(s.contains("…"), "{s}");
            assert!(s.contains("15:00"), "{s}");
            live.sources[0] = SourceState::Done(Outcome::Nothing("couldn't fetch"));
            live.sources[1] = SourceState::Checking;
            assert_eq!(video_box_rows(&working_screen(&live, w, 40)), size);
            live.sources[1] = SourceState::Done(Outcome::Nothing("no punctuation"));
            let s = working_screen(&live, w, 40);
            assert_eq!(video_box_rows(&s), size, "{s}");
            for want in ["couldn't fetch", "no punctuation"] {
                assert!(s.contains(want), "missing {want:?} at {w}:\n{s}");
            }
        }
    }

    #[test]
    fn cut_ends_with_an_ellipsis_only_when_it_cuts() {
        assert_eq!(cut("Wes Roth", 20), "Wes Roth");
        assert_eq!(cut("Wes Roth", 8), "Wes Roth");
        assert_eq!(cut("Wes Roth", 7), "Wes Ro…");
        // No space left dangling before the "…".
        assert_eq!(cut("Wes Roth", 5), "Wes…");
        assert_eq!(cut("Wes Roth", 0), "…");
    }

    #[test]
    fn a_short_screen_still_shows_every_stage_and_stop() {
        let s = working_screen(&listening(), 59, 18);
        for want in [
            "▸ Loading speech model",
            "○ Copy to your clipboard",
            "q  Stop",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        assert!(!s.contains("Model:"), "{s}");
    }

    #[test]
    fn says_where_youtube_words_came_from_at_phone_width() {
        let t = transcript(Source::AutoCaptions("en-orig".into()));
        let trail = Trail {
            captions: Outcome::Nothing("none"),
            auto: Outcome::Used,
            mac: Outcome::NotNeeded,
        };
        let s = screen(&t, trail, None, 59, 40);
        for want in [
            "VIDEO",
            "TRANSCRIPTION TAKEN FROM",
            "1 ✗ Human-written captions",
            "2 ✓ YouTube's automatic transcript (en)",
            "USED",
            "3 – Listening on this Mac",
            "not needed",
            "✓ Copied to your clipboard",
            "TRANSCRIPT",
            "9 words",
            "A bridge is always doing two jobs at once.",
            "Copy",
            "Quit",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
        assert!(!s.contains("Model:"), "{s}");
    }

    #[test]
    fn names_the_model_when_made_on_this_mac() {
        let t = transcript(Source::Parakeet);
        let trail = Trail {
            captions: Outcome::Nothing("none"),
            auto: Outcome::Nothing("no punctuation"),
            mac: Outcome::Used,
        };
        let s = screen(&t, trail, Some(30.0), 59, 40);
        for want in [
            "CREATED USING",
            "no punctuation",
            "3 ✓ Listening on this Mac",
            "Model: NVIDIA Parakeet 0.6B, English",
            "Transcribed 15:00 of audio in 0:30",
        ] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
    }

    #[test]
    fn a_short_screen_keeps_room_to_read() {
        let t = transcript(Source::AutoCaptions("en".into()));
        let trail = Trail {
            captions: Outcome::Nothing("none"),
            auto: Outcome::Used,
            mac: Outcome::NotNeeded,
        };
        let s = screen(&t, trail, None, 59, 18);
        assert!(s.contains("USED"), "{s}");
        assert!(!s.contains("not needed"), "{s}");
        assert!(
            s.contains("A bridge is always doing two jobs at once."),
            "{s}"
        );
    }

    #[test]
    fn a_narrow_phone_moves_how_it_went_underneath_and_keeps_quit() {
        let t = transcript(Source::Parakeet);
        let trail = Trail {
            captions: Outcome::Nothing("couldn't fetch"),
            auto: Outcome::Nothing("no punctuation"),
            mac: Outcome::Used,
        };
        let s = screen(&t, trail, Some(30.0), 45, 40);
        for want in ["couldn't fetch", "no punctuation", "USED", "Quit"] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
    }

    #[test]
    fn a_wide_desktop_window_keeps_a_readable_width_in_the_middle() {
        let t = transcript(Source::AutoCaptions("en".into()));
        let trail = Trail {
            captions: Outcome::Nothing("none"),
            auto: Outcome::Used,
            mac: Outcome::NotNeeded,
        };
        let s = screen(&t, trail, None, 200, 40);
        let top = s.lines().next().unwrap();
        let left = top.chars().take_while(|c| *c == ' ').count();
        assert_eq!(left, 50, "{top:?}");
        assert_eq!(top.trim_end().chars().count(), 150, "{top:?}");
    }

    fn youtube_trail() -> Trail {
        Trail {
            captions: Outcome::Nothing("none"),
            auto: Outcome::Used,
            mac: Outcome::NotNeeded,
        }
    }

    #[test]
    fn a_very_narrow_screen_still_shows_the_language() {
        let t = transcript(Source::AutoCaptions("en-orig".into()));
        let s = screen(&t, youtube_trail(), None, 39, 40);
        for want in ["(en)", "USED"] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
    }

    #[test]
    fn a_long_channel_name_keeps_the_length_in_view() {
        let mut t = transcript(Source::AutoCaptions("en".into()));
        t.channel = Some("A channel name that is very long indeed xx".into());
        let s = screen(&t, youtube_trail(), None, 45, 40);
        for want in ["very long indeed", "15:00"] {
            assert!(s.contains(want), "missing {want:?} in:\n{s}");
        }
    }

    #[test]
    fn the_tiniest_screen_still_shows_quit_whole() {
        let t = transcript(Source::AutoCaptions("en".into()));
        let s = screen(&t, youtube_trail(), None, 24, 40);
        let keys = s.lines().last().unwrap();
        assert!(keys.contains("q  Quit"), "{keys:?}");
    }

    #[test]
    fn a_very_short_screen_still_keeps_four_lines_to_read() {
        let t = transcript(Source::AutoCaptions("en".into()));
        let s = screen(&t, youtube_trail(), None, 59, 12);
        let rows: Vec<&str> = s.lines().collect();
        let top = rows.iter().position(|r| r.contains("TRANSCRIPT")).unwrap();
        let bottom = rows.iter().rposition(|r| r.starts_with('╰')).unwrap();
        // Rows strictly between the TRANSCRIPT box's top and bottom edges.
        let inside = bottom - top - 1;
        assert!(inside >= 4, "{s}");
        assert!(s.contains("USED"), "{s}");
    }
}
