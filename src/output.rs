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

use crate::error::CliError;

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

/// Whether `--verbose` was set; gates GraphQL request tracing.
static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Record the global `--verbose` flag (called once at startup).
pub fn set_verbose(on: bool) {
    VERBOSE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether verbose tracing is enabled.
pub fn is_verbose() -> bool {
    VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

/// When `--verbose` is set, dump an outgoing GraphQL request (URL, query, and
/// variables) to stderr so it never mixes with `--output json` on stdout.
pub fn trace_graphql(url: &str, query: &str, variables: &serde_json::Value) {
    if !is_verbose() {
        return;
    }
    trace_graphql_to(&mut io::stderr().lock(), url, query, variables);
}

/// Render a GraphQL trace to an arbitrary writer (testable core of
/// [`trace_graphql`]).
fn trace_graphql_to(w: &mut dyn Write, url: &str, query: &str, variables: &serde_json::Value) {
    let _ = writeln!(w, "--- GraphQL request → {} ---", url);
    let _ = writeln!(w, "{}", query.trim());
    match serde_json::to_string_pretty(variables) {
        Ok(vars) => {
            let _ = writeln!(w, "variables: {}", vars);
        }
        Err(_) => {
            let _ = writeln!(w, "variables: <unserializable>");
        }
    }
    let _ = writeln!(w, "--- end GraphQL request ---");
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

/// Resolve an error to its user-facing message.
///
/// If the outermost error is a [`CliError`] — either constructed directly
/// (`bail!(CliError::...)`) or installed as context (`.context(CliError::...)?`)
/// — its `Display` is shown verbatim. That's the convention: the outermost
/// context is the user-relevant story, deeper causes are technical detail.
///
/// Otherwise the error is internal: hidden behind a generic message, or
/// shown raw when `APPSIGNAL_CLI_DEBUG=1` is set.
fn sanitize_error_message(error: &Error) -> String {
    if let Some(user) = error.downcast_ref::<CliError>() {
        return user.to_string();
    }

    if debug_errors_enabled() {
        error.to_string()
    } else {
        "Something went wrong inside appsignal-cli. Try again, and rerun with `APPSIGNAL_CLI_DEBUG=1` if you need the internal error details.".to_string()
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
        print_error, render_boxed_lines, sanitize_error_message, trace_graphql_to, Output,
    };
    use crate::error::CliError;
    use anyhow::{anyhow, Context};

    #[test]
    fn trace_graphql_to_dumps_url_query_and_variables() {
        let mut buf = Vec::new();
        trace_graphql_to(
            &mut buf,
            "https://appsignal.com/graphql",
            "  query Ping { __typename }  ",
            &serde_json::json!({ "appId": "app-1" }),
        );
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("https://appsignal.com/graphql"));
        assert!(out.contains("query Ping { __typename }"));
        assert!(out.contains("\"appId\": \"app-1\""));
    }

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
    fn print_error_writes_without_error() {
        let err: anyhow::Error = CliError::msg("Friendly error").into();

        assert!(print_error(&err, Output::Human).is_ok());
        assert!(print_error(&err, Output::Json).is_ok());
    }

    #[test]
    fn sanitize_shows_clierror_verbatim() {
        let err: anyhow::Error = CliError::AuthRejected { detail: None }.into();

        assert_eq!(
            sanitize_error_message(&err),
            "Authentication failed. Your AppSignal credentials were rejected. Run `appsignal-cli auth login` again."
        );
    }

    #[test]
    fn sanitize_finds_clierror_through_chain() {
        // Mirrors the real usage: `Result::context(CliError::msg(...))?` on a
        // failing internal call. The CliError context must surface as the
        // user-facing message, with the inner cause available under DEBUG.
        let inner: Result<(), std::io::Error> = Err(std::io::Error::other("boom"));
        let err = inner
            .context(CliError::msg("Application not found"))
            .unwrap_err();

        assert_eq!(sanitize_error_message(&err), "Application not found");
    }

    #[test]
    fn sanitize_hides_unknown_errors() {
        // SAFETY: tests run in a single-threaded slice for env access here.
        // SAFE: env::remove_var is only unsafe in Rust 2024 multi-threaded contexts; ok in cfg(test).
        std::env::remove_var("APPSIGNAL_CLI_DEBUG");

        let err = anyhow!("database pool poisoned: internal sentinel");

        assert!(sanitize_error_message(&err).contains("Something went wrong inside appsignal-cli"));
    }
}
