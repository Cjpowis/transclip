//! Full-screen scrollable viewer. c copies again, q quits.

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::{Transcript, clipboard, text};

const GREEN: Color = Color::Rgb(80, 200, 120);
const AMBER: Color = Color::Rgb(255, 184, 0);
const RED: Color = Color::Rgb(235, 90, 90);
const MUTED: Color = Color::Rgb(120, 140, 160);

struct App<'a> {
    t: &'a Transcript,
    text_only: bool,
    scroll: usize,
    wrapped_for: u16,
    lines: Vec<String>,
    page: usize,
    status: (String, Color),
}

impl App<'_> {
    fn rewrap(&mut self, width: u16) {
        if width == self.wrapped_for {
            return;
        }
        self.wrapped_for = width;
        self.lines.clear();
        let w = width.max(10) as usize;
        for (i, p) in self.t.paragraphs.iter().enumerate() {
            if i > 0 {
                self.lines.push(String::new());
            }
            self.lines
                .extend(textwrap::wrap(p, w).into_iter().map(|l| l.into_owned()));
        }
    }

    fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.page.max(1))
    }

    fn scroll_by(&mut self, delta: isize) {
        let next = self.scroll as isize + delta;
        self.scroll = next.clamp(0, self.max_scroll() as isize) as usize;
    }

    fn copy(&mut self) {
        let chars = self.t.clipboard_text(self.text_only).chars().count();
        self.status = match clipboard::copy(&self.t.clipboard_text(self.text_only)) {
            Ok(()) => (
                format!("✓ Copied again ({} characters)", text::thousands(chars)),
                GREEN,
            ),
            Err(e) => (format!("✗ Couldn't copy: {e}"), RED),
        };
    }
}

/// `copied`: None if auto-copy was turned off, otherwise whether it worked.
pub fn run(t: &Transcript, copied: Option<bool>, text_only: bool) -> Result<()> {
    let status = match copied {
        Some(true) => ("✓ Copied to your clipboard".to_string(), GREEN),
        Some(false) => ("! Couldn't copy. Press c to try again".to_string(), AMBER),
        None => ("Press c to copy".to_string(), MUTED),
    };
    let mut app = App {
        t,
        text_only,
        scroll: 0,
        wrapped_for: 0,
        lines: Vec::new(),
        page: 1,
        status,
    };
    let mut terminal = ratatui::init();
    let _ = execute!(stdout(), EnableMouseCapture);
    let result = event_loop(&mut terminal, &mut app);
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
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

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let t = app.t;

    // Header: title (up to two lines), then details in muted blue-grey.
    let title_w = area.width.max(10) as usize;
    let mut header: Vec<Line> = textwrap::wrap(&t.title, title_w)
        .into_iter()
        .take(2)
        .map(|l| Line::from(l.into_owned()).bold())
        .collect();
    let mut details = Vec::new();
    if let Some(c) = &t.channel {
        details.push(c.clone());
    }
    if let Some(d) = t.duration {
        details.push(text::clock(d));
    }
    header.push(Line::from(details.join(" · ")).fg(MUTED));
    header.push(
        Line::from(format!(
            "{} · {} words",
            t.source_label(),
            text::thousands(t.words())
        ))
        .fg(MUTED),
    );
    let header_h = header.len() as u16 + 1;

    let [head, body, foot] = Layout::vertical([
        Constraint::Length(header_h),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(area);

    f.render_widget(
        Paragraph::new(header).block(
            Block::new()
                .borders(Borders::BOTTOM)
                .border_style(Style::new().fg(MUTED)),
        ),
        head,
    );

    // Body, with a one-column margin each side.
    let inner = Rect {
        x: body.x + 1,
        width: body.width.saturating_sub(2),
        ..body
    };
    app.rewrap(inner.width);
    app.page = inner.height as usize;
    app.scroll = app.scroll.min(app.max_scroll());
    let visible: Vec<Line> = app
        .lines
        .iter()
        .skip(app.scroll)
        .take(inner.height as usize)
        .map(|l| Line::from(l.as_str()))
        .collect();
    f.render_widget(Paragraph::new(visible), inner);

    // Footer: status and position, then the keys.
    let [status_row, keys_row] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(foot);
    let pct = if app.max_scroll() == 0 {
        100
    } else {
        app.scroll * 100 / app.max_scroll()
    };
    let [left, right] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(5)]).areas(status_row);
    f.render_widget(
        Paragraph::new(Span::styled(&app.status.0, Style::new().fg(app.status.1))),
        left,
    );
    f.render_widget(
        Paragraph::new(Line::from(format!("{pct}%")).right_aligned().fg(MUTED)),
        right,
    );
    f.render_widget(
        Paragraph::new(Line::from("c copy · ↑↓ scroll · q quit").fg(MUTED)),
        keys_row,
    );
}
