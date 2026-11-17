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

/// Join snippets into paragraphs, breaking at pauses and at sentence ends
/// once a paragraph gets long.
pub fn paragraphs(items: &[Timed]) -> Vec<String> {
    const PAUSE: f32 = 1.5;
    const LONG_PAUSE: f32 = 3.0;
    const SOFT_MAX: usize = 600;
    const HARD_MAX: usize = 1200;

    let mut paras = Vec::new();
    let mut cur = String::new();
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
                paras.push(std::mem::take(&mut cur));
            } else {
                cur.push(' ');
            }
        }
        cur.push_str(t);
        last_end = Some(it.end);
    }
    if !cur.is_empty() {
        paras.push(cur);
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
            vec!["Hello there. How are you?", "New topic."]
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
