//! Output rendering for CLI commands.
//!
//! Convention (enforced by `clippy.toml` at the repo root):
//!
//! - Command *results* go through [`print`]. Never `println!`.
//! - Status messages (progress, prompts, "OK") use the [`crate::status`] macro
//!   so they always land on stderr and don't pollute `--output json`.
//! - The global `--output` flag picks the format; commands don't decide.

#![allow(clippy::disallowed_macros)]

use std::io::{self, Write};

use anyhow::Result;
use clap::ValueEnum;
use serde::{Serialize, Serializer};
use tabled::{Table, Tabled};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Output {
    #[default]
    Human,
    Json,
}

/// Anything a command produces as its result.
///
/// JSON falls out of `Serialize` for free; only the human view is hand-rolled.
pub trait Render: Serialize {
    fn render_human(&self, w: &mut dyn Write) -> io::Result<()>;
}

struct CustomRender<T, F> {
    value: T,
    render_human: F,
}

impl<T: Serialize, F> Serialize for CustomRender<T, F> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(serializer)
    }
}

impl<T: Serialize, F> Render for CustomRender<T, F>
where
    F: Fn(&mut dyn Write) -> io::Result<()>,
{
    fn render_human(&self, w: &mut dyn Write) -> io::Result<()> {
        (self.render_human)(w)
    }
}

/// The single entry point for printing command results.
pub fn print<T: Render>(value: &T, format: Output) -> Result<()> {
    let stdout = io::stdout();
    let mut w = stdout.lock();
    match format {
        Output::Human => value.render_human(&mut w)?,
        Output::Json => {
            serde_json::to_writer_pretty(&mut w, value)?;
            writeln!(w)?;
        }
    }
    Ok(())
}

/// Print a command result without a bespoke named `Render` type.
pub fn print_with<T, F>(value: T, format: Output, render_human: F) -> Result<()>
where
    T: Serialize,
    F: Fn(&mut dyn Write) -> io::Result<()>,
{
    print(
        &CustomRender {
            value,
            render_human,
        },
        format,
    )
}

/// Render rows as a table. Compose inside a `Render::render_human` impl.
pub fn table<T: Tabled>(w: &mut dyn Write, rows: impl IntoIterator<Item = T>) -> io::Result<()> {
    writeln!(w, "{}", Table::new(rows))
}

/// Write a single JSON value followed by a newline.
pub fn json_line<T: Serialize>(w: &mut dyn Write, value: &T) -> Result<()> {
    serde_json::to_writer(&mut *w, value)?;
    writeln!(w)?;
    Ok(())
}

fn render_boxed_lines<T: AsRef<str>>(lines: &[T]) -> String {
    let width = lines
        .iter()
        .map(|line| line.as_ref().len())
        .max()
        .unwrap_or(0);
    let mut output = String::new();

    output.push('+');
    output.push_str(&"-".repeat(width + 2));
    output.push('+');
    output.push('\n');

    for line in lines {
        output.push_str(&format!("| {:<width$} |\n", line.as_ref(), width = width));
    }

    output.push('+');
    output.push_str(&"-".repeat(width + 2));
    output.push('+');

    output
}

/// Print a boxed status message to stderr.
pub fn status_box<T: AsRef<str>>(lines: &[T]) {
    crate::status!("{}", render_boxed_lines(lines));
}

/// Render key/value pairs as a detail panel. For "show one thing" commands.
#[allow(dead_code)] // part of the demonstrated surface; first caller lands in the migration
pub fn detail(w: &mut dyn Write, pairs: &[(&str, &str)]) -> io::Result<()> {
    let width = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in pairs {
        writeln!(w, "{:<width$}  {}", format!("{}:", k), v, width = width + 1)?;
    }
    Ok(())
}

/// Print a status message to stderr — progress, prompts, confirmations.
///
/// Routes through one macro so we can later swap in colored output or
/// `indicatif` without touching every callsite.
#[macro_export]
macro_rules! status {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let stderr = std::io::stderr();
        let mut stderr = stderr.lock();
        let _ = writeln!(stderr, $($arg)*);
    }};
}

#[cfg(test)]
mod tests {
    use super::render_boxed_lines;

    #[test]
    fn render_boxed_lines_wraps_content_in_ascii_box() {
        let rendered = render_boxed_lines(&[
            "Newer appsignal-cli version available",
            "Current: 0.2.0",
            "Latest:  0.3.0",
        ]);

        assert!(rendered.starts_with('+'));
        assert!(rendered.contains("| Newer appsignal-cli version available |"));
        assert!(rendered.contains("Current: 0.2.0"));
        assert!(rendered.contains("Latest:  0.3.0"));
        assert!(rendered.ends_with('+'));
    }
}
