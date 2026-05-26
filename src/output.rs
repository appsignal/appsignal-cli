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

use anyhow::{Error, Result};
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

#[derive(Serialize)]
struct ErrorResponse<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a str>,
}

fn debug_errors_enabled() -> bool {
    std::env::var_os("APPSIGNAL_CLI_DEBUG").is_some()
}

fn is_known_user_error(message: &str) -> bool {
    [
        "Authentication failed.",
        "AppSignal could not find the requested resource.",
        "AppSignal rate limited this request.",
        "AppSignal is unavailable right now.",
        "AppSignal request failed with HTTP",
        "AppSignal rejected the request.",
        "Could not reach AppSignal.",
        "AppSignal returned an unexpected response.",
        "No app found",
        "Multiple apps match",
        "No organization configured.",
        "Not authenticated.",
        "Invalid ",
        "Missing ",
        "OAuth ",
        "Token cannot be empty",
        "Multiple users match",
        "No log view found",
        "Multiple log views match",
        "Provide either --app-id or --app",
        "Please upgrade appsignal-cli to continue.",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

fn sanitize_error_message(error: &Error) -> String {
    let message = error.to_string();

    match message.as_str() {
        msg if is_known_user_error(msg) => msg.to_string(),
        msg if msg.starts_with("Failed to send request to AppSignal") => {
            "Could not reach AppSignal. Check your network connection and try again.".to_string()
        }
        msg if msg.starts_with("Failed to parse AppSignal response")
            || msg.starts_with("No data in AppSignal GraphQL response") =>
        {
            "AppSignal returned an unexpected response. Please try again.".to_string()
        }
        msg if msg.starts_with("Could not determine config directory")
            || msg.starts_with("Could not determine current directory")
            || msg.starts_with("Failed to read config at")
            || msg.starts_with("Failed to parse config at")
            || msg.starts_with("Failed to create config dir")
            || msg.starts_with("Failed to serialize config")
            || msg.starts_with("Failed to write config to") =>
        {
            "Could not read or write the appsignal-cli config. Check file permissions and try again.".to_string()
        }
        msg if msg.starts_with("Invalid OAuth redirect URI configured")
            || msg.starts_with("Loopback OAuth redirect URI must include a port")
            || msg.starts_with("Failed to bind local OAuth callback listener")
            || msg.starts_with("Timed out waiting for OAuth callback")
            || msg.starts_with("Failed to read OAuth callback request")
            || msg.starts_with("OAuth callback request was not valid UTF-8") =>
        {
            "OAuth login did not complete successfully. Try `appsignal-cli auth login --oauth` again.".to_string()
        }
        _ if debug_errors_enabled() => message,
        _ => {
            "Something went wrong inside appsignal-cli. Try again, and rerun with `APPSIGNAL_CLI_DEBUG=1` if you need the internal error details.".to_string()
        }
    }
}

fn error_details(error: &Error) -> Option<String> {
    if !debug_errors_enabled() {
        return None;
    }

    let details: Vec<String> = error
        .chain()
        .skip(1)
        .map(|cause| cause.to_string())
        .collect();
    if details.is_empty() {
        None
    } else {
        Some(details.join("\ncaused by: "))
    }
}

/// Print a user-facing command error.
pub fn print_error(error: &Error, format: Output) -> Result<()> {
    let message = sanitize_error_message(error);
    let details = error_details(error);

    match format {
        Output::Human => {
            let stderr = io::stderr();
            let mut w = stderr.lock();
            writeln!(w, "Error: {}", message)?;
            if let Some(details) = details {
                writeln!(w, "caused by: {}", details)?;
            }
        }
        Output::Json => {
            let stderr = io::stderr();
            let mut w = stderr.lock();
            serde_json::to_writer_pretty(
                &mut w,
                &ErrorResponse {
                    error: &message,
                    details: details.as_deref(),
                },
            )?;
            writeln!(w)?;
        }
    }

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
    use super::{
        debug_errors_enabled, print_error, render_boxed_lines, sanitize_error_message, Output,
    };
    use anyhow::anyhow;

    #[test]
    fn render_boxed_lines_wraps_content_in_ascii_box() {
        let rendered =
            render_boxed_lines(&["Upgrade required", "Current: 0.2.1", "Latest:  1.0.0"]);

        assert!(rendered.starts_with('+'));
        assert!(rendered.contains("| Upgrade required |"));
        assert!(rendered.contains("Current: 0.2.1"));
        assert!(rendered.contains("Latest:  1.0.0"));
        assert!(rendered.ends_with('+'));
    }

    #[test]
    fn print_error_returns_ok_for_human_output() {
        let err = anyhow!("Friendly error");

        assert!(print_error(&err, Output::Human).is_ok());
    }

    #[test]
    fn print_error_returns_ok_for_json_output() {
        let err = anyhow!("Friendly error");

        assert!(print_error(&err, Output::Json).is_ok());
    }

    #[test]
    fn sanitize_error_message_preserves_known_user_errors() {
        let err = anyhow!("Authentication failed. Your AppSignal credentials were rejected.");

        assert_eq!(
            sanitize_error_message(&err),
            "Authentication failed. Your AppSignal credentials were rejected."
        );
    }

    #[test]
    fn sanitize_error_message_hides_unknown_errors_without_debug() {
        if debug_errors_enabled() {
            return;
        }

        let err = anyhow!("database pool poisoned: internal sentinel");

        assert!(sanitize_error_message(&err).contains("Something went wrong inside appsignal-cli"));
    }

    #[test]
    fn sanitize_error_message_maps_transport_failures() {
        let err = anyhow!("Failed to send request to AppSignal");

        assert_eq!(
            sanitize_error_message(&err),
            "Could not reach AppSignal. Check your network connection and try again."
        );
    }
}
