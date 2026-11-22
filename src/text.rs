//! Turning timed snippets into readable paragraphs, plus small formatters.

/// A piece of text with its position in the video, in seconds.
pub struct Timed {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// Drop control characters (terminal escape codes and the like) from text
/// that came from the internet, so it can't drive the terminal when printed.
/// Newlines and tabs stay; other control characters that separate words
/// (carriage returns, form feeds) become spaces.
pub fn strip_controls(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\t' => Some(c),
            c if c.is_control() && c.is_whitespace() => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

/// A chapter of the video: where it starts, in seconds, and its title.
pub struct Chapter {
    pub start: f32,
    pub title: String,
}

/// A piece of the finished transcript.
pub enum Block {
    /// A chapter's title.
    Heading(String),
    /// A paragraph, and where in the video it starts, in seconds.
    Para { start: f32, text: String },
}

/// Paragraphs, with a heading where each chapter starts. Paragraphs never
/// run across a chapter break. Chapters are only used when there are two or
/// more, and a chapter with no speech gets no heading.
pub fn blocks(items: &[Timed], chapters: &[Chapter]) -> Vec<Block> {
    // Captions often start a moment before the chapter mark they belong to.
    const LEAD: f32 = 0.5;
    let chapters: &[Chapter] = if chapters.len() >= 2 { chapters } else { &[] };
    let until = |from: usize, t: f32| {
        from + items[from..]
            .iter()
            .position(|it| it.start >= t - LEAD)
            .unwrap_or(items.len() - from)
    };
    let paras = |slice: &[Timed]| {
        paragraphs(slice)
            .into_iter()
            .map(|(start, text)| Block::Para { start, text })
            .collect::<Vec<_>>()
    };

    // Anything before the first chapter (rare) comes first, untitled.
    let mut i = until(0, chapters.first().map_or(f32::INFINITY, |c| c.start));
    let mut out = paras(&items[..i]);
    for (n, ch) in chapters.iter().enumerate() {
        let end = until(i, chapters.get(n + 1).map_or(f32::INFINITY, |c| c.start));
        let body = paras(&items[i..end]);
        if !body.is_empty() {
            out.push(Block::Heading(ch.title.clone()));
            out.extend(body);
        }
        i = end;
    }
    out
}

/// Join snippets into paragraphs, breaking at pauses and at sentence ends
/// once a paragraph gets long. Each comes with where it starts, in seconds.
pub fn paragraphs(items: &[Timed]) -> Vec<(f32, String)> {
    const PAUSE: f32 = 1.5;
    const LONG_PAUSE: f32 = 3.0;
    const SOFT_MAX: usize = 600;
    const HARD_MAX: usize = 1200;

    let mut paras = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 0.0;
    let mut last_end: Option<f32> = None;

    for it in items {
        let t = it.text.trim();
        if t.is_empty() {
            continue;
        }
        if !cur.is_empty() {
            let gap = last_end.map_or(0.0, |e| it.start - e);
            let sentence_done = cur.ends_with(['.', '?', '!', '…', '"', ')', ']']);
            let brk = gap >= LONG_PAUSE
                || (sentence_done && (gap >= PAUSE || cur.len() >= SOFT_MAX))
                || cur.len() >= HARD_MAX;
            if brk {
                paras.push((cur_start, std::mem::take(&mut cur)));
            } else {
                cur.push(' ');
            }
        }
        if cur.is_empty() {
            cur_start = it.start;
        }
        cur.push_str(t);
        last_end = Some(it.end);
    }
    if !cur.is_empty() {
        paras.push((cur_start, cur));
    }
    paras
}

/// 75.0 -> "1:15", 3725.0 -> "1:02:05"
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// 12345 -> "12,345"
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escape_codes_but_keeps_text() {
        assert_eq!(
            strip_controls("a\x1b]52;c;aGk=\x07b\tc\nd\u{9b}2Je"),
            "a]52;c;aGk=b\tc\nd2Je"
        );
        assert_eq!(strip_controls("Café ♪ “quotes”"), "Café ♪ “quotes”");
        assert_eq!(strip_controls("Hello\rthere\x0cnow"), "Hello there now");
    }

    fn t(start: f32, end: f32, text: &str) -> Timed {
        Timed {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn joins_close_snippets_and_breaks_on_pause() {
        let items = [
            t(0.0, 1.0, "Hello there."),
            t(1.0, 2.0, "How are you?"),
            t(4.0, 5.0, "New topic."),
        ];
        assert_eq!(
            paragraphs(&items),
            vec![
                (0.0, "Hello there. How are you?".to_string()),
                (4.0, "New topic.".to_string())
            ]
        );
    }

    fn chapter(start: f32, title: &str) -> Chapter {
        Chapter {
            start,
            title: title.into(),
        }
    }

    /// Headings and paragraphs as plain strings, for comparing.
    fn shape(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b {
                Block::Heading(h) => format!("# {h}"),
                Block::Para { start, text } => format!("{start}: {text}"),
            })
            .collect()
    }

    #[test]
    fn puts_headings_where_chapters_start_and_breaks_paragraphs_there() {
        let items = [
            t(0.0, 1.0, "Welcome"),
            t(1.0, 2.0, "to the show"),
            // Starts a moment before the 2 s mark, so it belongs to chapter 2.
            t(1.8, 3.0, "First point"),
            t(3.0, 4.0, "goes on"),
            t(10.0, 11.0, "Second point."),
        ];
        let chapters = [
            chapter(0.0, "Intro"),
            chapter(2.0, "Point one"),
            chapter(6.0, "Nothing said here"),
            chapter(9.0, "Point two"),
        ];
        assert_eq!(
            shape(&blocks(&items, &chapters)),
            vec![
                "# Intro",
                "0: Welcome to the show",
                "# Point one",
                "1.8: First point goes on",
                "# Point two",
                "10: Second point.",
            ]
        );
    }

    #[test]
    fn a_single_chapter_or_none_means_no_headings() {
        let items = [t(0.0, 1.0, "Just talk.")];
        assert_eq!(shape(&blocks(&items, &[])), vec!["0: Just talk."]);
        assert_eq!(
            shape(&blocks(&items, &[chapter(0.0, "Only one")])),
            vec!["0: Just talk."]
        );
    }

    #[test]
    fn speech_before_the_first_chapter_comes_first_untitled() {
        let items = [t(0.0, 1.0, "Cold open."), t(5.0, 6.0, "Main bit.")];
        let chapters = [chapter(4.0, "Main"), chapter(20.0, "End")];
        assert_eq!(
            shape(&blocks(&items, &chapters)),
            vec!["0: Cold open.", "# Main", "5: Main bit."]
        );
    }

    #[test]
    fn long_pause_breaks_even_mid_sentence() {
        let items = [
            t(0.0, 1.0, "no punctuation here"),
            t(5.0, 6.0, "still none"),
        ];
        assert_eq!(paragraphs(&items).len(), 2);
    }

    #[test]
    fn formats() {
        assert_eq!(clock(75.0), "1:15");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(thousands(12345), "12,345");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }
}
