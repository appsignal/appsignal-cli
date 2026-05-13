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
use serde::Serialize;
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

/// Render rows as a table. Compose inside a `Render::render_human` impl.
pub fn table<T: Tabled>(w: &mut dyn Write, rows: impl IntoIterator<Item = T>) -> io::Result<()> {
    writeln!(w, "{}", Table::new(rows))
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
        eprintln!($($arg)*);
    }};
}
