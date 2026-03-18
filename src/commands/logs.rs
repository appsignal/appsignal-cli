use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::time::{sleep, Duration};

use super::{authenticated_client, resolve_org};
use crate::api::{AppSignalClient, LogLine, LogView};
use crate::config::Config;

/// Tail (stream) log lines for an application, polling every second.
/// Supports filtering by query, severities, source IDs, and log view.
#[allow(clippy::too_many_arguments)]
pub async fn tail(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    query: Option<&str>,
    severities: Option<&str>,
    source_ids: Option<&str>,
    view: Option<&str>,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    // Resolve log view filters if --view is given
    let (view_query, view_source_ids, view_severities) = if let Some(view_name) = view {
        let lv = resolve_log_view(&client, &resolved_app_id, view_name).await?;
        (
            lv.query.clone(),
            lv.source_ids.clone(),
            lv.severities.clone(),
        )
    } else {
        (None, None, None)
    };

    // Merge CLI flags with view defaults (CLI flags take precedence)
    let effective_query = query.map(|q| q.to_string()).or(view_query);
    let effective_source_ids = source_ids
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>()
        })
        .or(view_source_ids);
    let effective_severities = severities
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_uppercase())
                .collect::<Vec<_>>()
        })
        .or(view_severities);

    eprintln!(
        "Tailing logs{} (Ctrl+C to stop)...",
        if let Some(v) = view {
            format!(" [view: {}]", v)
        } else {
            String::new()
        }
    );

    // Start 60 seconds in the past for initial context
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut start = Utc::now() - chrono::Duration::seconds(60);

    loop {
        let end = Utc::now();
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        let lines = client
            .list_log_lines(
                &resolved_app_id,
                Some(&start_str),
                Some(&end_str),
                effective_source_ids.as_deref(),
                effective_severities.as_deref(),
                effective_query.as_deref(),
                Some(100),
                Some("ASC"),
            )
            .await?;

        for line in &lines {
            if seen_ids.insert(line.id.clone()) {
                print_log_line(line);
            }
        }

        // Move the window forward: use 2-minute lookback from now for dedup safety
        start = end - chrono::Duration::seconds(120);

        // Prune seen_ids to avoid unbounded growth (keep last 500)
        if seen_ids.len() > 1000 {
            // Just clear and re-seed from current batch
            seen_ids.clear();
            for line in &lines {
                seen_ids.insert(line.id.clone());
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

/// Search log lines (one-shot query). Designed for both human use and LLM consumption.
/// With --page-all, automatically paginates by slicing the time window to collect all results.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    query: Option<&str>,
    severities: Option<&str>,
    source_ids: Option<&str>,
    view: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    limit: Option<i64>,
    order: Option<&str>,
    json_output: bool,
    page_all: bool,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    // Resolve log view filters if --view is given
    let (view_query, view_source_ids, view_severities) = if let Some(view_name) = view {
        let lv = resolve_log_view(&client, &resolved_app_id, view_name).await?;
        (
            lv.query.clone(),
            lv.source_ids.clone(),
            lv.severities.clone(),
        )
    } else {
        (None, None, None)
    };

    // Merge CLI flags with view defaults (CLI flags take precedence)
    let effective_query = query.map(|q| q.to_string()).or(view_query);
    let effective_source_ids = source_ids
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>()
        })
        .or(view_source_ids);
    let effective_severities = severities
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_uppercase())
                .collect::<Vec<_>>()
        })
        .or(view_severities);

    let lines = if page_all {
        fetch_all_pages(
            &client,
            &resolved_app_id,
            start,
            end,
            effective_source_ids.as_deref(),
            effective_severities.as_deref(),
            effective_query.as_deref(),
        )
        .await?
    } else {
        client
            .list_log_lines(
                &resolved_app_id,
                start,
                end,
                effective_source_ids.as_deref(),
                effective_severities.as_deref(),
                effective_query.as_deref(),
                limit,
                order,
            )
            .await?
    };

    if json_output {
        let json = serde_json::to_string_pretty(&lines)
            .context("Failed to serialize log lines to JSON")?;
        println!("{}", json);
    } else {
        if lines.is_empty() {
            println!("No log lines found.");
            return Ok(());
        }

        for line in &lines {
            print_log_line(line);
        }

        eprintln!("\n{} log line(s) returned.", lines.len());
    }

    Ok(())
}

/// Fetch all log lines in a time range by paginating with time-window slicing.
/// Fetches in ASC order, and when a page returns 100 results (the API cap),
/// uses the last timestamp as the start of the next page.
/// Deduplicates by log line ID to handle boundary overlap.
async fn fetch_all_pages(
    client: &AppSignalClient,
    app_id: &str,
    start: Option<&str>,
    end: Option<&str>,
    source_ids: Option<&[String]>,
    severities: Option<&[String]>,
    query: Option<&str>,
) -> Result<Vec<LogLine>> {
    let mut all_lines: Vec<LogLine> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut current_start = start.map(|s| s.to_string());
    let page_limit: i64 = 100;
    let mut page = 0;

    loop {
        page += 1;
        let batch = client
            .list_log_lines(
                app_id,
                current_start.as_deref(),
                end,
                source_ids,
                severities,
                query,
                Some(page_limit),
                Some("ASC"),
            )
            .await?;

        let batch_len = batch.len();

        for line in batch {
            if seen_ids.insert(line.id.clone()) {
                all_lines.push(line);
            }
        }

        eprintln!(
            "Page {}: fetched {} lines ({} total unique)",
            page,
            batch_len,
            all_lines.len()
        );

        // If we got fewer than the limit, we've exhausted the range
        if batch_len < page_limit as usize {
            break;
        }

        // Use the last line's timestamp as the new start boundary
        let last_timestamp = all_lines
            .last()
            .map(|l| l.timestamp.clone())
            .context("Unexpected empty result after pagination")?;
        current_start = Some(last_timestamp);
    }

    Ok(all_lines)
}

/// List all log views (saved filter presets) for an app.
pub async fn views(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let views = client.list_log_views(&resolved_app_id).await?;

    if views.is_empty() {
        println!("No log views found.");
        return Ok(());
    }

    println!("{:<28} {:<40} {:<20} SEVERITIES", "ID", "NAME", "QUERY");
    println!("{}", "-".repeat(100));

    for v in &views {
        let query_str = v.query.as_deref().unwrap_or("-");
        let sevs = v
            .severities
            .as_ref()
            .map(|s| s.join(","))
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<28} {:<40} {:<20} {}",
            truncate(&v.id, 26),
            truncate(&v.name, 38),
            truncate(query_str, 18),
            sevs,
        );
    }

    println!("\n{} log view(s) found.", views.len());
    Ok(())
}

/// List all log sources for an app.
pub async fn sources(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let sources = client.list_log_sources(&resolved_app_id).await?;

    if sources.is_empty() {
        println!("No log sources found.");
        return Ok(());
    }

    println!("{:<28} {:<30} {:<12} FORMAT", "ID", "NAME", "TYPE");
    println!("{}", "-".repeat(80));

    for s in &sources {
        println!(
            "{:<28} {:<30} {:<12} {}",
            truncate(&s.id, 26),
            truncate(&s.name, 28),
            s.kind.as_deref().unwrap_or("-"),
            s.fmt.as_deref().unwrap_or("-"),
        );
    }

    println!("\n{} log source(s) found.", sources.len());
    Ok(())
}

// -- Helpers --

/// Resolve a log view by name or ID. Tries ID first, then case-insensitive name match.
async fn resolve_log_view(
    client: &AppSignalClient,
    app_id: &str,
    name_or_id: &str,
) -> Result<LogView> {
    // Try by ID first
    if let Ok(view) = client.get_log_view(app_id, name_or_id).await {
        return Ok(view);
    }

    // Fall back to name match
    let views = client.list_log_views(app_id).await?;
    let name_lower = name_or_id.to_lowercase();
    let matches: Vec<&LogView> = views
        .iter()
        .filter(|v| v.name.to_lowercase() == name_lower)
        .collect();

    match matches.len() {
        0 => anyhow::bail!(
            "No log view found matching '{}'. Use `logs views` to list available views.",
            name_or_id
        ),
        1 => Ok(matches[0].clone()),
        _ => {
            let descriptions: Vec<String> = matches
                .iter()
                .map(|v| format!("  {} ({})", v.id, v.name))
                .collect();
            anyhow::bail!(
                "Multiple log views match '{}'. Use the view ID to disambiguate:\n{}",
                name_or_id,
                descriptions.join("\n")
            )
        }
    }
}

fn print_log_line(line: &LogLine) {
    let source_name = line
        .source
        .as_ref()
        .and_then(|s| s.name.as_deref())
        .unwrap_or("-");

    let attrs = line
        .attributes
        .as_ref()
        .map(|attrs| {
            if attrs.is_empty() {
                String::new()
            } else {
                let kv: Vec<String> = attrs
                    .iter()
                    .map(|a| format!("{}={}", a.key, a.value.as_deref().unwrap_or("")))
                    .collect();
                format!(" [{}]", kv.join(" "))
            }
        })
        .unwrap_or_default();

    println!(
        "{} {:<8} {:<16} {:<24} {}{}",
        &line.timestamp,
        line.severity.to_uppercase(),
        source_name,
        line.hostname,
        line.message,
        attrs,
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AppSignalClient, KeyStringValue, LogSourceRef};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_print_log_line_basic() {
        // Just ensure it doesn't panic
        let line = LogLine {
            id: "abc".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            severity: "error".to_string(),
            hostname: "web-1".to_string(),
            group: None,
            message: "Something went wrong".to_string(),
            attributes: Some(vec![KeyStringValue {
                key: "request_id".to_string(),
                value: Some("123".to_string()),
            }]),
            source: Some(LogSourceRef {
                id: "s1".to_string(),
                name: Some("Application".to_string()),
            }),
        };
        print_log_line(&line);
    }

    #[test]
    fn test_print_log_line_no_source() {
        let line = LogLine {
            id: "def".to_string(),
            timestamp: "2025-06-15T12:30:00Z".to_string(),
            severity: "info".to_string(),
            hostname: "worker-3".to_string(),
            group: Some("background".to_string()),
            message: "Job completed".to_string(),
            attributes: None,
            source: None,
        };
        print_log_line(&line);
    }

    #[test]
    fn test_print_log_line_empty_attributes() {
        let line = LogLine {
            id: "ghi".to_string(),
            timestamp: "2025-06-15T12:30:00Z".to_string(),
            severity: "warn".to_string(),
            hostname: "api-1".to_string(),
            group: None,
            message: "Rate limited".to_string(),
            attributes: Some(vec![]),
            source: Some(LogSourceRef {
                id: "s2".to_string(),
                name: None,
            }),
        };
        print_log_line(&line);
    }

    // -- Helper to build log line JSON for wiremock responses --

    fn log_line_json(
        id: &str,
        timestamp: &str,
        severity: &str,
        message: &str,
    ) -> serde_json::Value {
        json!({
            "id": id,
            "timestamp": timestamp,
            "severity": severity,
            "hostname": "web-1",
            "group": null,
            "message": message,
            "attributes": [],
            "source": null
        })
    }

    fn graphql_log_response(lines: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "data": {
                "app": {
                    "logs": {
                        "lines": lines
                    }
                }
            }
        })
    }

    // -- fetch_all_pages tests --

    #[tokio::test]
    async fn test_fetch_all_pages_single_page() {
        // When results < 100, should return in one page
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_log_response(vec![
                    log_line_json("l1", "2025-06-01T12:00:00Z", "INFO", "msg1"),
                    log_line_json("l2", "2025-06-01T12:01:00Z", "ERROR", "msg2"),
                ])),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = fetch_all_pages(
            &client,
            "app1",
            Some("2025-06-01T00:00:00Z"),
            Some("2025-06-02T00:00:00Z"),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, "l1");
        assert_eq!(lines[1].id, "l2");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(graphql_log_response(vec![])))
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = fetch_all_pages(&client, "app1", None, None, None, None, None)
            .await
            .unwrap();

        assert!(lines.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_all_pages_multi_page() {
        // Simulate pagination: first call returns 100 lines, second returns 30
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First page: exactly 100 lines (triggers pagination)
                    let lines: Vec<serde_json::Value> = (0..100)
                        .map(|i| {
                            log_line_json(
                                &format!("page1-{}", i),
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("message {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(graphql_log_response(lines))
                } else {
                    // Second page: fewer than 100 (terminates pagination)
                    let lines: Vec<serde_json::Value> = (0..30)
                        .map(|i| {
                            log_line_json(
                                &format!("page2-{}", i),
                                &format!("2025-06-01T13:{:02}:00Z", i % 60),
                                "ERROR",
                                &format!("page2 message {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(graphql_log_response(lines))
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = fetch_all_pages(
            &client,
            "app1",
            Some("2025-06-01T00:00:00Z"),
            Some("2025-06-02T00:00:00Z"),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert_eq!(lines.len(), 130); // 100 + 30
        assert_eq!(lines[0].id, "page1-0");
        assert_eq!(lines[99].id, "page1-99");
        assert_eq!(lines[100].id, "page2-0");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_deduplicates() {
        // Second page includes some IDs from first page (boundary overlap)
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First page: exactly 100 lines
                    let lines: Vec<serde_json::Value> = (0..100)
                        .map(|i| {
                            log_line_json(
                                &format!("line-{}", i),
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("msg {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(graphql_log_response(lines))
                } else {
                    // Second page: 5 duplicates from page 1 + 15 new lines = 20 total
                    let mut lines: Vec<serde_json::Value> = (95..100)
                        .map(|i| {
                            log_line_json(
                                &format!("line-{}", i), // duplicate IDs
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("msg {}", i),
                            )
                        })
                        .collect();
                    for i in 0..15 {
                        lines.push(log_line_json(
                            &format!("new-{}", i),
                            &format!("2025-06-01T13:{:02}:00Z", i % 60),
                            "WARN",
                            &format!("new msg {}", i),
                        ));
                    }
                    ResponseTemplate::new(200).set_body_json(graphql_log_response(lines))
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = fetch_all_pages(&client, "app1", None, None, None, None, None)
            .await
            .unwrap();

        // 100 from page 1 + 15 new from page 2 (5 dupes removed)
        assert_eq!(lines.len(), 115);
        // Verify no duplicate IDs
        let ids: HashSet<String> = lines.iter().map(|l| l.id.clone()).collect();
        assert_eq!(ids.len(), 115);
    }
}
