//! Reading YouTube's json3 caption files.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::text::Timed;

#[derive(Deserialize)]
struct Json3 {
    #[serde(default)]
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "tStartMs")]
    start_ms: Option<f64>,
    #[serde(rename = "dDurationMs")]
    dur_ms: Option<f64>,
    segs: Option<Vec<Seg>>,
}

#[derive(Deserialize)]
struct Seg {
    utf8: Option<String>,
}

pub fn load(path: &Path) -> Result<Vec<Timed>> {
    let raw = std::fs::read_to_string(path).context("couldn't read the captions file")?;
    parse(&raw)
}

pub fn parse(raw: &str) -> Result<Vec<Timed>> {
    let j: Json3 = serde_json::from_str(raw).context("captions weren't in the expected format")?;
    let mut out: Vec<Timed> = Vec::new();
    for e in j.events {
        let Some(segs) = e.segs else { continue };
        let joined: String = segs.iter().filter_map(|s| s.utf8.as_deref()).collect();
        let text = crate::text::strip_controls(&joined)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() || out.last().is_some_and(|l| l.text == text) {
            continue;
        }
        let start = e.start_ms.unwrap_or(0.0) / 1000.0;
        let end = start + e.dur_ms.unwrap_or(0.0) / 1000.0;
        out.push(Timed {
            start: start as f32,
            end: end as f32,
            text,
        });
    }
    Ok(out)
}

/// YouTube's automatic transcripts have full stops and capitals on newer
/// videos but are one long lowercase stream on older ones. Count it as
/// punctuated if there's at least one sentence ending per 100 words.
/// Full-width endings (as in Chinese and Japanese) count too.
pub fn is_punctuated(items: &[Timed]) -> bool {
    let words: usize = items
        .iter()
        .map(|i| i.text.split_whitespace().count())
        .sum();
    let ends: usize = items
        .iter()
        .map(|i| {
            i.text
                .chars()
                .filter(|c| matches!(c, '.' | '?' | '!' | '。' | '？' | '！'))
                .count()
        })
        .sum();
    words >= 20 && ends * 100 >= words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str) -> Timed {
        Timed {
            start: 0.0,
            end: 1.0,
            text: text.into(),
        }
    }

    #[test]
    fn tells_punctuated_transcripts_from_lowercase_streams() {
        let lines = [
            "so in college I was a government major which means I had to",
            "write a lot of papers now when a normal student writes a paper",
            "they might spread the work out a little like this so you know",
        ];
        let plain: Vec<Timed> = lines.iter().map(|l| t(l)).collect();
        assert!(!is_punctuated(&plain));
        let punctuated: Vec<Timed> = lines.iter().map(|l| t(&format!("{l}."))).collect();
        assert!(is_punctuated(&punctuated));
        assert!(!is_punctuated(&[t("Too short.")]));
    }

    #[test]
    fn keeps_words_apart_and_drops_escape_codes() {
        let raw = r#"{"events":[
            {"tStartMs":0,"dDurationMs":1000,"segs":[{"utf8":"Hello\rthere \u001b[2Jfriend"}]}
        ]}"#;
        let items = parse(raw).unwrap();
        assert_eq!(items[0].text, "Hello there [2Jfriend");
    }

    #[test]
    fn parses_events_and_skips_empty_and_repeats() {
        let raw = r#"{"events":[
            {"tStartMs":0,"dDurationMs":2000,"id":1,"wpWinPosId":1},
            {"tStartMs":0,"dDurationMs":2000,"segs":[{"utf8":"Hello\nthere."}]},
            {"tStartMs":2000,"dDurationMs":1000,"segs":[{"utf8":"Hello there."}]},
            {"tStartMs":3000,"dDurationMs":1500,"segs":[{"utf8":"(Laughter)"}]},
            {"tStartMs":5000,"segs":[{"utf8":"\n"}]}
        ]}"#;
        let items = parse(raw).unwrap();
        let texts: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, vec!["Hello there.", "(Laughter)"]);
        assert_eq!(items[1].start, 3.0);
        assert_eq!(items[1].end, 4.5);
    }
}
