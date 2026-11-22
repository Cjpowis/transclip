//! Progress lines on stderr: spinners for short steps, bars for long ones.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub const GREEN: &str = "\x1b[32m";
pub const AMBER: &str = "\x1b[33m";
pub const RED: &str = "\x1b[31m";
pub const MUTED: &str = "\x1b[38;2;120;140;160m";
pub const RESET: &str = "\x1b[0m";

pub fn spinner(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .expect("valid template")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// A bar out of 1000, so fractions map straight onto it.
pub fn bar(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new(1000);
    pb.set_style(
        ProgressStyle::with_template("{msg} {bar:18.green/blue} {percent:>3}% {prefix}")
            .expect("valid template")
            .progress_chars("█▌░"),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(250));
    pb
}

/// The furthest a bar goes before the job says it's finished. The bar rounds
/// its percentage to a whole number, so 99.5% and up would already read 100%.
pub const NEARLY_DONE: f64 = 0.99;

/// Where a bar should be between progress reports. Transcribing only reports
/// after each piece of audio, so without this the bar jumps a piece at a time
/// and sits still in between.
pub struct Glide {
    /// The smallest share of the whole job one piece can be, so the bar never
    /// passes the end of the piece being worked on.
    pub piece: f64,
    /// Share of the job done per second, guessed, until the first report.
    pub guess_rate: f64,
}

impl Glide {
    /// `done` is the share reported finished, `at` the seconds in when it was
    /// reported, and `now` the seconds in now. Returns the share to show, and
    /// the seconds left once a report has measured the speed.
    pub fn at(&self, done: f64, at: f64, now: f64) -> (f64, Option<f64>) {
        let measured = done > 0.0 && at > 0.0;
        let rate = if measured { done / at } else { self.guess_rate };
        // Never past the end of the piece being worked on, and never 100%
        // before the job says it's finished.
        let most = if done < 1.0 { NEARLY_DONE } else { 1.0 };
        let least = done.min(most);
        let limit = (done + self.piece).clamp(least, most);
        let shown = (done + rate * (now - at).max(0.0)).clamp(least, limit);
        let left = (measured && rate > 0.0).then(|| (1.0 - shown) / rate);
        (shown, left)
    }
}

pub fn done(pb: &ProgressBar, msg: impl AsRef<str>) {
    finish(pb, format!("{GREEN}✓{RESET} {}", msg.as_ref()));
}

pub fn warn(pb: &ProgressBar, msg: impl AsRef<str>) {
    finish(pb, format!("{AMBER}!{RESET} {}", msg.as_ref()));
}

fn finish(pb: &ProgressBar, line: String) {
    // With stderr going to a file or pipe the bar draws nothing, so the result
    // line would be lost. Print it plainly instead.
    if pb.is_hidden() {
        pb.finish_and_clear();
        eprintln!("{line}");
        return;
    }
    pb.set_style(ProgressStyle::with_template("{msg}").expect("valid template"));
    pb.finish_with_message(line);
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Glide = Glide {
        piece: 0.05,
        guess_rate: 0.02,
    };

    #[test]
    fn glides_at_the_measured_speed() {
        // 10% done after 1 second: 10% a second, so 15% half a second later.
        let (shown, left) = G.at(0.1, 1.0, 1.5);
        assert!((shown - 0.15).abs() < 1e-9, "{shown}");
        assert!((left.unwrap() - 8.5).abs() < 1e-9, "{left:?}");
    }

    #[test]
    fn waits_at_the_end_of_the_piece() {
        let (shown, _) = G.at(0.1, 1.0, 60.0);
        assert!((shown - 0.15).abs() < 1e-9, "{shown}");
    }

    #[test]
    fn never_shows_100_percent_early() {
        let (shown, left) = G.at(0.98, 9.8, 60.0);
        assert!((shown - NEARLY_DONE).abs() < 1e-9, "{shown}");
        assert!(left.unwrap() > 0.0);
        // As drawn: the bar is out of 1000, and indicatif's {percent} is
        // `fraction * 100` as f32, rounded to a whole number.
        let pos = (shown * 1000.0) as u64;
        assert_eq!(format!("{:.0}", pos as f32 / 1000.0 * 100.0), "99");
    }

    #[test]
    fn holds_at_99_percent_even_when_reported_further() {
        // A long video's last piece can be under half a percent of it.
        let (shown, _) = G.at(0.996, 10.0, 10.0);
        assert!((shown - NEARLY_DONE).abs() < 1e-9, "{shown}");
    }

    #[test]
    fn guesses_before_the_first_report_without_a_time() {
        let (shown, left) = G.at(0.0, 0.0, 1.0);
        assert!((shown - 0.02).abs() < 1e-9, "{shown}");
        assert_eq!(left, None);
    }

    #[test]
    fn finished_is_finished() {
        assert_eq!(G.at(1.0, 30.0, 30.0).0, 1.0);
    }
}
