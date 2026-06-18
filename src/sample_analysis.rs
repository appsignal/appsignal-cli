//! Turn a raw transaction [`Sample`] into the digest investigators actually
//! read.
//!
//! A raw sample is large and mostly noise; the questions a responder asks are
//! always the same: what ran, how slow was it, who hit it, which queries
//! dominated, was there an N+1, and (for errors) what blew up and what led to
//! it. [`analyze`] distils a sample into a [`SampleAnalysis`] answering those,
//! and [`SampleAnalysis::render_digest`] prints it.
//!
//! Everything here is pure (no I/O, no API calls) so it is exhaustively unit
//! tested on synthetic samples.

use std::io::{self, Write};

use serde::Serialize;

use crate::api::{BacktraceLine, Sample};

/// How many slow events / queries / backtrace frames to surface.
const TOP_N: usize = 10;
/// A group is treated as database work if its name contains one of these.
const DB_MARKERS: [&str; 6] = ["sql", "query", "active_record", "ecto", "db.", "mongo"];
/// Keys (case-insensitive) that identify the acting user, in priority order.
const USER_KEYS: [&str; 6] = [
    "user_id",
    "user",
    "current_user",
    "user_email",
    "email",
    "username",
];
/// Keys (case-insensitive) that identify the request.
const REQUEST_ID_KEYS: [&str; 3] = ["request_id", "request-id", "request id"];
/// Minimum repeats of one query fingerprint before it's an N+1 suspect.
const N_PLUS_ONE_THRESHOLD: i64 = 3;

/// The distilled, investigator-facing view of a sample.
#[derive(Debug, Serialize)]
pub struct SampleAnalysis {
    pub overview: Overview,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostic_tags: Vec<Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceAnalysis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorAnalysis>,
}

#[derive(Debug, Serialize)]
pub struct Overview {
    #[serde(rename = "type")]
    pub sample_type: String,
    pub incident_number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Tag {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct PerformanceAnalysis {
    pub has_n_plus_one: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub breakdown: Vec<GroupStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<GroupStat>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slowest_events: Vec<EventStat>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slow_queries: Vec<SlowQuery>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub n_plus_one_suspects: Vec<NPlusOneSuspect>,
    /// How many timeline events the API dropped before returning the sample.
    /// When present, the breakdown and slowest-events lists are understated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated_events: Option<i64>,
}

/// Per-group rollup: how much of the request a group of events accounts for.
#[derive(Debug, Serialize)]
pub struct GroupStat {
    pub group: String,
    pub count: i64,
    pub total_ms: f64,
    pub avg_ms: f64,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct EventStat {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub duration_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct SlowQuery {
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub duration_ms: f64,
}

/// A query fingerprint that recurred enough times to suggest an N+1.
#[derive(Debug, Serialize)]
pub struct NPlusOneSuspect {
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub count: i64,
    pub total_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct ErrorAnalysis {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub backtrace: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<CauseLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub breadcrumbs: Vec<BreadcrumbLine>,
}

#[derive(Debug, Serialize)]
pub struct CauseLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BreadcrumbLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Analyse a sample into its digest form. `sample_type` is the incident type
/// reported by the API (`"performance"` / `"error"`); it decides whether the
/// performance or error section is produced.
pub fn analyze(sample: &Sample, sample_type: &str, incident_number: i64) -> SampleAnalysis {
    let overview = build_overview(sample, sample_type, incident_number);
    let diagnostic_tags = build_tags(sample);

    let (performance, error) = if sample_type == "error" || sample.exception.is_some() {
        (None, Some(build_error(sample)))
    } else {
        (Some(build_performance(sample)), None)
    };

    SampleAnalysis {
        overview,
        diagnostic_tags,
        performance,
        error,
    }
}

fn build_overview(sample: &Sample, sample_type: &str, incident_number: i64) -> Overview {
    Overview {
        sample_type: sample_type.to_string(),
        incident_number,
        action: sample.action.clone(),
        namespace: sample.namespace.clone(),
        occurred_at: sample.created_at.clone(),
        duration_ms: sample.duration.map(round2),
        queue_ms: sample.queue_duration.map(round2),
        revision: sample.revision.clone(),
        user: sample_user(sample),
        request_id: find_in_keyvalues(sample, &REQUEST_ID_KEYS),
    }
}

/// Best-effort identity of the user who triggered the sample, looked up across
/// the curated `overview`/`attributes` key/values and then the `sessionData` /
/// `customData` / `params` JSON. Used for the digest and for `--user` filtering.
pub fn sample_user(sample: &Sample) -> Option<String> {
    if let Some(value) = find_in_keyvalues(sample, &USER_KEYS) {
        return Some(value);
    }
    for json in [&sample.session_data, &sample.custom_data, &sample.params] {
        if let Some(value) = find_in_json(json.as_ref(), &USER_KEYS) {
            return Some(value);
        }
    }
    None
}

/// Find the first key (case-insensitive) from `keys` in a JSON object and
/// return its scalar value as a string.
fn find_in_json(value: Option<&serde_json::Value>, keys: &[&str]) -> Option<String> {
    let object = value?.as_object()?;
    for key in keys {
        for (k, v) in object {
            if k.eq_ignore_ascii_case(key) {
                if let Some(scalar) = json_scalar_to_string(v) {
                    return Some(scalar);
                }
            }
        }
    }
    None
}

fn json_scalar_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// AppSignal's `overview` is its own curated high-signal key/value summary, so
/// surface it directly as the diagnostic tags.
fn build_tags(sample: &Sample) -> Vec<Tag> {
    let Some(overview) = &sample.overview else {
        return Vec::new();
    };
    overview
        .iter()
        .filter_map(|kv| {
            kv.value
                .as_ref()
                .filter(|v| !v.is_empty())
                .map(|value| Tag {
                    key: kv.key.clone(),
                    value: value.clone(),
                })
        })
        .collect()
}

fn build_performance(sample: &Sample) -> PerformanceAnalysis {
    let timeline = sample.timeline.as_deref().unwrap_or(&[]);

    let breakdown = group_breakdown(timeline);
    let database = database_rollup(&breakdown);
    let slowest_events = slowest_events(timeline);
    let slow_queries = slow_queries(timeline);
    let n_plus_one_suspects = n_plus_one_suspects(timeline);
    let has_n_plus_one = sample.has_n_plus_one == Some(true) || !n_plus_one_suspects.is_empty();
    // Only flag truncation when the API actually dropped events (> 0).
    let truncated_events = sample.timeline_truncated_events.filter(|n| *n > 0);

    PerformanceAnalysis {
        has_n_plus_one,
        breakdown,
        database,
        slowest_events,
        slow_queries,
        n_plus_one_suspects,
        truncated_events,
    }
}

/// Roll the timeline up by group, sorted by total time descending. Percentages
/// are of the timeline's own total, so they're meaningful regardless of how
/// event durations relate to wall-clock.
fn group_breakdown(timeline: &[crate::api::TimelineEvent]) -> Vec<GroupStat> {
    use std::collections::HashMap;

    let mut totals: HashMap<&str, (i64, f64)> = HashMap::new();
    for event in timeline {
        let group = event.group.as_deref().unwrap_or("(ungrouped)");
        let entry = totals.entry(group).or_insert((0, 0.0));
        entry.0 += event.count.unwrap_or(1).max(1);
        entry.1 += event.duration.unwrap_or(0.0);
    }

    let grand_total: f64 = totals.values().map(|(_, total)| *total).sum();

    let mut stats: Vec<GroupStat> = totals
        .into_iter()
        .map(|(group, (count, total))| GroupStat {
            group: group.to_string(),
            count,
            total_ms: round2(total),
            avg_ms: round2(if count > 0 { total / count as f64 } else { 0.0 }),
            percent: round2(percent_of(total, grand_total)),
        })
        .collect();

    stats.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
    stats
}

/// Combine all database-ish groups into a single rollup.
fn database_rollup(breakdown: &[GroupStat]) -> Option<GroupStat> {
    let db: Vec<&GroupStat> = breakdown.iter().filter(|s| is_database(&s.group)).collect();
    if db.is_empty() {
        return None;
    }
    let count: i64 = db.iter().map(|s| s.count).sum();
    let total: f64 = db.iter().map(|s| s.total_ms).sum();
    let percent: f64 = db.iter().map(|s| s.percent).sum();
    Some(GroupStat {
        group: "database".to_string(),
        count,
        total_ms: round2(total),
        avg_ms: round2(if count > 0 { total / count as f64 } else { 0.0 }),
        percent: round2(percent),
    })
}

fn slowest_events(timeline: &[crate::api::TimelineEvent]) -> Vec<EventStat> {
    let mut events: Vec<EventStat> = timeline
        .iter()
        .filter_map(|event| {
            event.duration.map(|duration| EventStat {
                name: event_name(event),
                group: event.group.clone(),
                duration_ms: round2(duration),
            })
        })
        .collect();
    events.sort_by(|a, b| b.duration_ms.total_cmp(&a.duration_ms));
    events.truncate(TOP_N);
    events
}

fn slow_queries(timeline: &[crate::api::TimelineEvent]) -> Vec<SlowQuery> {
    let mut queries: Vec<SlowQuery> = timeline
        .iter()
        .filter_map(|event| {
            let body = event.payload.as_ref().and_then(|p| p.body.as_ref())?;
            if body.is_empty() {
                return None;
            }
            Some(SlowQuery {
                body: body.clone(),
                group: event.group.clone(),
                duration_ms: round2(event.duration.unwrap_or(0.0)),
            })
        })
        .collect();
    queries.sort_by(|a, b| b.duration_ms.total_cmp(&a.duration_ms));
    queries.truncate(TOP_N);
    queries
}

/// Find query fingerprints (`digest`) that recur often enough to look like an
/// N+1: either many distinct events share the digest, or a single event's
/// `count` is high.
fn n_plus_one_suspects(timeline: &[crate::api::TimelineEvent]) -> Vec<NPlusOneSuspect> {
    use std::collections::HashMap;

    let mut by_digest: HashMap<&str, (i64, f64, Option<String>)> = HashMap::new();
    for event in timeline {
        let Some(digest) = event.digest.as_deref() else {
            continue;
        };
        if digest.is_empty() {
            continue;
        }
        let entry = by_digest
            .entry(digest)
            .or_insert((0, 0.0, event.group.clone()));
        entry.0 += event.count.unwrap_or(1).max(1);
        entry.1 += event.duration.unwrap_or(0.0);
    }

    let mut suspects: Vec<NPlusOneSuspect> = by_digest
        .into_iter()
        .filter(|(_, (count, _, _))| *count >= N_PLUS_ONE_THRESHOLD)
        .map(|(digest, (count, total, group))| NPlusOneSuspect {
            digest: digest.to_string(),
            group,
            count,
            total_ms: round2(total),
        })
        .collect();

    suspects.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.total_ms.total_cmp(&a.total_ms))
    });
    suspects.truncate(TOP_N);
    suspects
}

fn build_error(sample: &Sample) -> ErrorAnalysis {
    let exception = sample.exception.as_ref();

    let backtrace = exception
        .and_then(|e| e.backtrace.as_ref())
        .map(|frames| {
            frames
                .iter()
                .take(TOP_N)
                .map(format_backtrace_frame)
                .collect()
        })
        .unwrap_or_default();

    let causes = sample
        .error_causes
        .as_ref()
        .map(|causes| {
            causes
                .iter()
                .map(|c| CauseLine {
                    name: c.name.clone(),
                    message: c.message.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let breadcrumbs = sample
        .breadcrumbs
        .as_ref()
        .map(|crumbs| {
            crumbs
                .iter()
                .map(|c| BreadcrumbLine {
                    category: c.category.clone(),
                    action: c.action.clone(),
                    message: c.message.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    ErrorAnalysis {
        exception: exception.and_then(|e| e.name.clone()),
        message: exception.and_then(|e| e.message.clone()),
        backtrace,
        causes,
        breadcrumbs,
    }
}

/// Format a backtrace frame for display. Shared by the digest and the raw
/// sample view.
pub fn format_backtrace_frame(frame: &BacktraceLine) -> String {
    if let Some(original) = &frame.original {
        if !original.is_empty() {
            return original.clone();
        }
    }
    let location = match (&frame.path, frame.line) {
        (Some(path), Some(line)) => format!("{}:{}", path, line),
        (Some(path), None) => path.clone(),
        _ => "?".to_string(),
    };
    match &frame.method {
        Some(method) => format!("{} in {}", location, method),
        None => location,
    }
}

fn event_name(event: &crate::api::TimelineEvent) -> String {
    event
        .name
        .clone()
        .or_else(|| event.action.clone())
        .or_else(|| event.group.clone())
        .unwrap_or_else(|| "(event)".to_string())
}

/// Find the first non-empty value whose key (case-insensitive) matches one of
/// `keys`, searching `overview` then `attributes`.
fn find_in_keyvalues(sample: &Sample, keys: &[&str]) -> Option<String> {
    let sources = [sample.overview.as_ref(), sample.attributes.as_ref()];
    for key in keys {
        for source in sources.iter().flatten() {
            for kv in source.iter() {
                if kv.key.eq_ignore_ascii_case(key) {
                    if let Some(value) = kv.value.as_ref().filter(|v| !v.is_empty()) {
                        return Some(value.clone());
                    }
                }
            }
        }
    }
    None
}

fn is_database(group: &str) -> bool {
    let lower = group.to_ascii_lowercase();
    DB_MARKERS.iter().any(|marker| lower.contains(marker))
}

fn percent_of(part: f64, total: f64) -> f64 {
    if total > 0.0 {
        part / total * 100.0
    } else {
        0.0
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

impl SampleAnalysis {
    /// Print the human-readable digest.
    pub fn render_digest(&self, w: &mut dyn Write) -> io::Result<()> {
        self.render_overview(w)?;

        if !self.diagnostic_tags.is_empty() {
            writeln!(w, "\nDiagnostics")?;
            for tag in &self.diagnostic_tags {
                writeln!(w, "  {}: {}", tag.key, tag.value)?;
            }
        }

        if let Some(performance) = &self.performance {
            performance.render(w)?;
        }
        if let Some(error) = &self.error {
            error.render(w)?;
        }

        Ok(())
    }

    fn render_overview(&self, w: &mut dyn Write) -> io::Result<()> {
        let o = &self.overview;
        writeln!(
            w,
            "{} sample · incident #{}",
            title_case(&o.sample_type),
            o.incident_number
        )?;
        write_opt(w, "Action", o.action.as_deref())?;
        write_opt(w, "Namespace", o.namespace.as_deref())?;
        write_opt(w, "Occurred at", o.occurred_at.as_deref())?;
        if let Some(duration) = o.duration_ms {
            writeln!(w, "  Duration:    {:.2} ms", duration)?;
        }
        if let Some(queue) = o.queue_ms {
            writeln!(w, "  Queue:       {:.2} ms", queue)?;
        }
        write_opt(w, "User", o.user.as_deref())?;
        write_opt(w, "Request ID", o.request_id.as_deref())?;
        write_opt(w, "Revision", o.revision.as_deref())?;
        Ok(())
    }
}

impl PerformanceAnalysis {
    fn render(&self, w: &mut dyn Write) -> io::Result<()> {
        if let Some(truncated) = self.truncated_events {
            writeln!(
                w,
                "\n⚠ {} timeline event(s) truncated by the API — the breakdown and \
                 slowest-events below are understated.",
                truncated
            )?;
        }

        if self.has_n_plus_one {
            writeln!(w, "\n⚠ N+1 query pattern detected")?;
        }

        if let Some(db) = &self.database {
            writeln!(
                w,
                "\nDatabase: {} queries, {:.2} ms total ({:.1}% of timeline), {:.2} ms avg",
                db.count, db.total_ms, db.percent, db.avg_ms
            )?;
        }

        if !self.breakdown.is_empty() {
            writeln!(w, "\nPerformance breakdown")?;
            writeln!(
                w,
                "  {:<32} {:>7} {:>12} {:>7}",
                "GROUP", "COUNT", "TOTAL", "%"
            )?;
            for stat in &self.breakdown {
                writeln!(
                    w,
                    "  {:<32} {:>7} {:>9.2} ms {:>6.1}%",
                    truncate(&stat.group, 32),
                    stat.count,
                    stat.total_ms,
                    stat.percent
                )?;
            }
        }

        if !self.slowest_events.is_empty() {
            writeln!(w, "\nSlowest events")?;
            for event in &self.slowest_events {
                writeln!(w, "  {:>9.2} ms  {}", event.duration_ms, event.name)?;
            }
        }

        if !self.n_plus_one_suspects.is_empty() {
            writeln!(w, "\nN+1 suspects (repeated query fingerprints)")?;
            for suspect in &self.n_plus_one_suspects {
                writeln!(
                    w,
                    "  {}× {:.2} ms  {}",
                    suspect.count,
                    suspect.total_ms,
                    truncate(&suspect.digest, 48)
                )?;
            }
        }

        if !self.slow_queries.is_empty() {
            writeln!(w, "\nSlow queries")?;
            for query in &self.slow_queries {
                writeln!(
                    w,
                    "  {:>9.2} ms  {}",
                    query.duration_ms,
                    truncate(&query.body, 100)
                )?;
            }
        }

        Ok(())
    }
}

impl ErrorAnalysis {
    fn render(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(
            w,
            "\nException:   {}",
            self.exception.as_deref().unwrap_or("-")
        )?;
        if let Some(message) = &self.message {
            writeln!(w, "Message:     {}", message)?;
        }

        if !self.backtrace.is_empty() {
            writeln!(w, "\nBacktrace")?;
            for frame in &self.backtrace {
                writeln!(w, "  {}", frame)?;
            }
        }

        if !self.causes.is_empty() {
            writeln!(w, "\nError causes")?;
            for cause in &self.causes {
                writeln!(
                    w,
                    "  {}: {}",
                    cause.name.as_deref().unwrap_or("-"),
                    cause.message.as_deref().unwrap_or("-")
                )?;
            }
        }

        if !self.breadcrumbs.is_empty() {
            writeln!(w, "\nBreadcrumbs (most recent last)")?;
            for crumb in &self.breadcrumbs {
                let category = crumb.category.as_deref().unwrap_or("-");
                let action = crumb.action.as_deref().unwrap_or("-");
                let message = crumb.message.as_deref().unwrap_or("");
                writeln!(w, "  [{}] {} {}", category, action, message)?;
            }
        }

        Ok(())
    }
}

fn write_opt(w: &mut dyn Write, label: &str, value: Option<&str>) -> io::Result<()> {
    if let Some(value) = value {
        writeln!(w, "  {}:{}{}", label, padding(label), value)?;
    }
    Ok(())
}

fn padding(label: &str) -> String {
    // Align values to a 12-column label gutter.
    let target = 12usize.saturating_sub(label.len() + 1);
    " ".repeat(target.max(1))
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let kept: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ExceptionDetail, KeyStringValue, TimelineEvent, TimelinePayload};

    fn kv(key: &str, value: &str) -> KeyStringValue {
        KeyStringValue {
            key: key.to_string(),
            value: Some(value.to_string()),
        }
    }

    fn event(group: &str, name: &str, duration: f64, digest: Option<&str>) -> TimelineEvent {
        TimelineEvent {
            name: Some(name.to_string()),
            action: None,
            group: Some(group.to_string()),
            duration: Some(duration),
            count: Some(1),
            digest: digest.map(str::to_string),
            payload: None,
        }
    }

    fn base_sample() -> Sample {
        Sample {
            id: "0123456789abcdef01234567-1".to_string(),
            action: Some("Web::OrdersController#index".to_string()),
            namespace: Some("web".to_string()),
            duration: Some(200.0),
            queue_duration: Some(5.0),
            created_at: Some("2026-05-19T14:30:00Z".to_string()),
            revision: Some("abc123".to_string()),
            version: None,
            original_id: None,
            has_n_plus_one: None,
            attributes: None,
            overview: Some(vec![kv("user_id", "user-42"), kv("request_id", "req-7")]),
            environment: None,
            params: None,
            session_data: None,
            custom_data: None,
            timeline: None,
            timeline_truncated_events: None,
            group_durations: None,
            group_allocations: None,
            exception: None,
            error_causes: None,
            breadcrumbs: None,
        }
    }

    #[test]
    fn overview_extracts_user_and_request_id_from_overview() {
        let analysis = analyze(&base_sample(), "performance", 12);
        assert_eq!(analysis.overview.user.as_deref(), Some("user-42"));
        assert_eq!(analysis.overview.request_id.as_deref(), Some("req-7"));
        assert_eq!(analysis.overview.duration_ms, Some(200.0));
    }

    #[test]
    fn performance_breakdown_groups_and_ranks_by_total() {
        let mut sample = base_sample();
        sample.timeline = Some(vec![
            event("sql.active_record", "User Load", 60.0, Some("d1")),
            event("sql.active_record", "Order Load", 30.0, Some("d2")),
            event("view.render", "index.html", 120.0, None),
        ]);
        let analysis = analyze(&sample, "performance", 12);
        let perf = analysis.performance.unwrap();

        // view.render (120) ranks above sql (90 combined across two events).
        assert_eq!(perf.breakdown[0].group, "view.render");
        assert_eq!(perf.breakdown[0].total_ms, 120.0);

        let db = perf.database.expect("database rollup");
        assert_eq!(db.count, 2);
        assert_eq!(db.total_ms, 90.0);
    }

    #[test]
    fn surfaces_timeline_truncation_in_analysis_and_digest() {
        let mut sample = base_sample();
        sample.timeline = Some(vec![event("view.render", "index.html", 120.0, None)]);
        sample.timeline_truncated_events = Some(37);
        let analysis = analyze(&sample, "performance", 12);
        let perf = analysis.performance.as_ref().unwrap();
        assert_eq!(perf.truncated_events, Some(37));

        let mut buf = Vec::new();
        analysis.render_digest(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("37 timeline event(s) truncated"));
        assert!(out.contains("understated"));
    }

    #[test]
    fn does_not_flag_truncation_when_zero_or_absent() {
        let mut sample = base_sample();
        sample.timeline = Some(vec![event("view.render", "index.html", 120.0, None)]);
        // Absent and explicit-zero both mean "nothing dropped".
        assert_eq!(
            analyze(&sample, "performance", 1)
                .performance
                .unwrap()
                .truncated_events,
            None
        );
        sample.timeline_truncated_events = Some(0);
        assert_eq!(
            analyze(&sample, "performance", 1)
                .performance
                .unwrap()
                .truncated_events,
            None
        );
    }

    #[test]
    fn detects_n_plus_one_from_repeated_digest() {
        let mut sample = base_sample();
        sample.timeline = Some(vec![
            event("sql.active_record", "Comment Load", 5.0, Some("same")),
            event("sql.active_record", "Comment Load", 5.0, Some("same")),
            event("sql.active_record", "Comment Load", 5.0, Some("same")),
        ]);
        let perf = analyze(&sample, "performance", 1).performance.unwrap();
        assert!(perf.has_n_plus_one);
        assert_eq!(perf.n_plus_one_suspects.len(), 1);
        assert_eq!(perf.n_plus_one_suspects[0].count, 3);
    }

    #[test]
    fn collects_slow_queries_from_payload_bodies() {
        let mut sample = base_sample();
        let mut slow = event("sql.active_record", "User Load", 120.0, Some("d1"));
        slow.payload = Some(TimelinePayload {
            name: Some("SQL".to_string()),
            body: Some("SELECT * FROM users WHERE id = ?".to_string()),
        });
        sample.timeline = Some(vec![slow]);
        let perf = analyze(&sample, "performance", 1).performance.unwrap();
        assert_eq!(perf.slow_queries.len(), 1);
        assert!(perf.slow_queries[0].body.contains("SELECT"));
        assert_eq!(perf.slow_queries[0].duration_ms, 120.0);
    }

    #[test]
    fn sample_user_falls_back_to_session_data_json() {
        let mut sample = base_sample();
        sample.overview = None;
        sample.session_data = Some(serde_json::json!({ "current_user": "alice@example.com" }));
        assert_eq!(sample_user(&sample).as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn error_analysis_includes_backtrace_and_causes() {
        let mut sample = base_sample();
        sample.exception = Some(ExceptionDetail {
            name: Some("RuntimeError".to_string()),
            message: Some("boom".to_string()),
            backtrace: Some(vec![BacktraceLine {
                line: Some(10),
                path: Some("app/models/order.rb".to_string()),
                method: Some("total".to_string()),
                column: None,
                original: None,
                kind: None,
                url: None,
            }]),
        });
        let analysis = analyze(&sample, "error", 99);
        assert!(analysis.performance.is_none());
        let error = analysis.error.unwrap();
        assert_eq!(error.exception.as_deref(), Some("RuntimeError"));
        assert_eq!(error.backtrace, vec!["app/models/order.rb:10 in total"]);
    }

    #[test]
    fn digest_renders_without_error() {
        let mut sample = base_sample();
        sample.timeline = Some(vec![event(
            "sql.active_record",
            "User Load",
            60.0,
            Some("d1"),
        )]);
        let analysis = analyze(&sample, "performance", 12);
        let mut buf = Vec::new();
        analysis.render_digest(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("incident #12"));
        assert!(out.contains("Performance breakdown"));
        assert!(out.contains("user-42"));
    }
}
