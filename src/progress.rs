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
