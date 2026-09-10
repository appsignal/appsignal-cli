use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use serde_json::{Map, Value};
use tokio::task::JoinSet;

use super::incidents::ExceptionErrorView;
use crate::api::{
    AppSignalClient, BacktraceFrameKind, BacktraceLine, Incident, TraceSpan, TraceSummary,
};
use crate::error::CliError;

#[derive(Default)]
pub(super) struct Enrichment {
    pub exception: Option<ExceptionErrorView>,
    pub causes: Vec<ParsedCause>,
    pub warning: Option<String>,
}

pub(super) struct ParsedCause {
    pub error: ExceptionErrorView,
    stacktrace_id: Option<String>,
}

struct ParsedException {
    exception: Option<ExceptionErrorView>,
    causes: Vec<ParsedCause>,
    revision: Option<String>,
}

#[derive(Debug, thiserror::Error)]
enum SelectionError {
    #[error("the selected span has no matching exception")]
    MissingSelectedSpan,
    #[error("the trace has no unique exception matching this incident")]
    AmbiguousException,
}

// Classify failures without exposing response bodies, URLs or exception payloads.
fn failure_reason(error: &anyhow::Error) -> String {
    if let Some(selection) = error.downcast_ref::<SelectionError>() {
        return selection.to_string();
    }
    if error.is::<tokio::task::JoinError>() {
        return "backtrace task failed".to_owned();
    }
    match error.downcast_ref::<CliError>() {
        Some(CliError::AuthRejected { .. }) => {
            "authentication rejected; run appsignal-cli auth login"
        }
        Some(CliError::OAuthScopeRejected) => "OAuth token lacks the required scope",
        Some(CliError::NotFound { .. }) => "requested resource was not found",
        Some(CliError::RateLimited { .. }) => "request was rate limited",
        Some(CliError::Unavailable { .. }) => "server is unavailable",
        Some(CliError::GraphQlRejected(_)) => "GraphQL query was rejected",
        Some(CliError::NetworkUnreachable) => "network request failed",
        Some(CliError::UnexpectedResponse) => "server returned an unexpected response",
        Some(CliError::AccountRestricted(_)) => "account access is restricted",
        Some(CliError::UnexpectedHttp { status, .. }) => return format!("HTTP {status}"),
        _ => "unexpected internal error",
    }
    .to_owned()
}

fn string(attributes: &Map<String, Value>, key: &str) -> Option<String> {
    attributes
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn chain_index(attributes: &Map<String, Value>) -> Option<(u64, u64)> {
    let value = attributes
        .get("appsignal.exception.stacktrace_index")?
        .as_str()?;
    let (event, cause) = value.split_once('_')?;
    if !event.bytes().all(|c| c.is_ascii_digit()) || !cause.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((event.parse().ok()?, cause.parse().ok()?))
}

fn error_view(attributes: &Map<String, Value>) -> ExceptionErrorView {
    ExceptionErrorView {
        name: string(attributes, "exception.type").unwrap_or_else(|| "Unknown error".into()),
        message: string(attributes, "exception.message"),
        first_line: None,
    }
}

fn parse_exception(
    digests: &[String],
    summary: &TraceSummary,
    spans: &[TraceSpan],
) -> Result<ParsedException> {
    let mut matches = Vec::new();
    for (span_index, span) in spans.iter().enumerate() {
        for (event_index, name) in span.event_names.iter().enumerate() {
            if name != "exception" {
                continue;
            }
            let Some(attributes) = span.event_attributes.get(event_index) else {
                continue;
            };
            if string(attributes, "appsignal.incident_digest")
                .is_some_and(|digest| digests.contains(&digest))
            {
                matches.push((span_index, event_index));
            }
        }
    }
    if let Some(span_id) = summary.span_id.as_deref().filter(|id| !id.is_empty()) {
        matches.retain(|(index, _)| spans[*index].span_id == span_id);
        if matches.is_empty() {
            return Err(SelectionError::MissingSelectedSpan.into());
        }
    }
    if matches.len() != 1 {
        return Err(SelectionError::AmbiguousException.into());
    }
    let (span_index, event_index) = matches[0];
    let span = &spans[span_index];
    let primary = &span.event_attributes[event_index];
    let mut candidates = Vec::new();
    if let Some((event, primary_cause)) = chain_index(primary) {
        for (index, name) in span.event_names.iter().enumerate() {
            let Some(attributes) = span.event_attributes.get(index) else {
                continue;
            };
            if name != "exception"
                || attributes
                    .get("appsignal.incident_digest")
                    .is_some_and(|value| {
                        !value.is_null()
                            && value
                                .as_str()
                                .is_none_or(|digest| !digest.trim().is_empty())
                    })
            {
                continue;
            }
            if let Some((candidate_event, cause)) = chain_index(attributes) {
                if candidate_event == event && cause > primary_cause {
                    candidates.push((cause, attributes));
                }
            }
        }
    }
    candidates.sort_by_key(|(index, _)| *index);
    // Duplicate cause positions are ambiguous; do not invent an ordering.
    let mut counts = HashMap::new();
    for (index, _) in &candidates {
        *counts.entry(*index).or_insert(0) += 1;
    }
    candidates.retain(|(index, _)| counts[index] == 1);
    Ok(ParsedException {
        exception: string(primary, "exception.type").map(|_| error_view(primary)),
        causes: candidates
            .iter()
            .map(|(_, attributes)| ParsedCause {
                error: error_view(attributes),
                stacktrace_id: string(attributes, "appsignal.stacktrace_id"),
            })
            .collect(),
        revision: string(&span.resource_attributes, "appsignal.config.revision"),
    })
}

fn first_app_line(lines: &[BacktraceLine]) -> Option<String> {
    let frame = lines
        .iter()
        .find(|frame| frame.kind == Some(BacktraceFrameKind::App))?;
    let clean = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    clean(&frame.original).or_else(|| {
        let path = clean(&frame.path)?;
        Some(match clean(&frame.line) {
            Some(line) => format!("{path}:{line}"),
            None => path,
        })
    })
}

pub(super) async fn fetch(
    client: &AppSignalClient,
    app_id: &str,
    incident: &Incident,
) -> Enrichment {
    fetch_with_deadline(client, app_id, incident, Duration::from_secs(10)).await
}

async fn fetch_with_deadline(
    client: &AppSignalClient,
    app_id: &str,
    incident: &Incident,
    deadline: Duration,
) -> Enrichment {
    let Incident::ExceptionIncident { digests, .. } = incident else {
        return Enrichment::default();
    };
    let mut seen = HashSet::new();
    let digests: Vec<String> = digests
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|digest| digest.trim())
        .filter(|digest| !digest.is_empty())
        .filter(|digest| seen.insert((*digest).to_owned()))
        .map(str::to_owned)
        .collect();
    let mut result = Enrichment::default();
    if digests.is_empty() {
        return result;
    }
    match tokio::time::timeout(deadline, enrich(client, app_id, &digests, &mut result)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            result.warning = Some(format!(
                "Could not load exception trace details ({}); showing available incident details.",
                failure_reason(&error)
            ));
        }
        Err(_) => {
            result.warning = Some(
                "Exception trace details timed out; showing available incident details.".into(),
            )
        }
    }
    result
}

async fn enrich(
    client: &AppSignalClient,
    app_id: &str,
    digests: &[String],
    result: &mut Enrichment,
) -> Result<()> {
    let summaries = client
        .list_error_traces_for_digests(app_id, digests, None, 1, "DESC", None)
        .await?;
    let Some(summary) = summaries.first() else {
        return Ok(());
    };
    let spans = client
        .get_error_trace_for_digests(app_id, digests, &summary.trace_id)
        .await?;
    let parsed = parse_exception(digests, summary, &spans)?;
    result.exception = parsed.exception;
    result.causes = parsed.causes;

    result.warning = enrich_backtraces(&mut result.causes, |id| {
        let client = client.clone();
        let app_id = app_id.to_owned();
        let revision = parsed.revision.clone();
        async move {
            client
                .get_backtrace(&app_id, &id, revision.as_deref())
                .await
        }
    })
    .await;
    Ok(())
}

async fn enrich_backtraces<F, Fut>(causes: &mut [ParsedCause], mut fetch: F) -> Option<String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Vec<BacktraceLine>>> + Send + 'static,
{
    let mut seen = HashSet::new();
    let ids: Vec<_> = causes
        .iter()
        .filter_map(|cause| cause.stacktrace_id.as_ref())
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect();
    let mut ids = ids.into_iter();
    let mut tasks = JoinSet::new();
    let mut warning = None;
    loop {
        while tasks.len() < 4 {
            let Some(id) = ids.next() else {
                break;
            };
            let response = fetch(id.clone());
            tasks.spawn(async move { (id, response.await) });
        }
        let Some(completed) = tasks.join_next().await else {
            break;
        };
        let response = completed
            .map_err(anyhow::Error::from)
            .and_then(|(id, response)| response.map(|lines| (id, lines)));
        match response {
            Ok((id, lines)) => {
                let first_line = first_app_line(&lines);
                for cause in causes.iter_mut() {
                    if cause.stacktrace_id.as_deref() == Some(&id) {
                        cause.error.first_line = first_line.clone();
                    }
                }
            }
            Err(error) => {
                let reason = failure_reason(&error);
                warning.get_or_insert_with(|| format!(
                    "Could not load some cause backtraces ({reason}); showing available exception details."
                ));
            }
        }
    }
    warning
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn incident() -> Incident {
        serde_json::from_value(json!({
            "__typename": "ExceptionIncident", "id": "i", "number": 7, "count": 1,
            "digests": [" digest-a ", "digest-b", "digest-a", ""]
        }))
        .unwrap()
    }

    fn summary() -> TraceSummary {
        serde_json::from_value(json!({"trace_id": "trace", "span_id": "span"})).unwrap()
    }

    fn event(name: &str, index: &str, digest: Option<&str>) -> Value {
        let mut value = json!({
            "exception.type": name, "exception.message": format!("{name} message"),
            "appsignal.exception.stacktrace_index": index,
            "appsignal.stacktrace_id": format!("stack-{name}")
        });
        if let Some(digest) = digest {
            value["appsignal.incident_digest"] = json!(digest);
        }
        value
    }

    fn span(id: &str, events: Vec<Value>) -> TraceSpan {
        serde_json::from_value(json!({
            "span_id": id, "trace_id": "trace",
            "events.name": events.iter().map(|_| "exception").collect::<Vec<_>>(),
            "events.attributes": events,
            "resource_attributes": {"appsignal.config.revision": "rev"}
        }))
        .unwrap()
    }

    fn parse(spans: &[TraceSpan]) -> Result<ParsedException> {
        parse_exception(&["digest-a".into(), "digest-b".into()], &summary(), spans)
    }

    #[test]
    fn causes_are_numeric_and_confined_to_the_primary_chain_and_span() {
        let main = span(
            "span",
            vec![
                event("Root", "1_0", Some("digest-b")),
                event("Tenth", "1_10", None),
                event("Second", "1_2", None),
                event("OtherChain", "10_1", None),
                event("OtherIncident", "1_1", Some("unrelated")),
                event("Malformed", "1_bad", None),
                event("Negative", "1_-1", None),
            ],
        );
        let other = span("other", vec![event("OtherSpan", "1_1", None)]);
        let result = parse(&[main, other]).unwrap();
        assert_eq!(result.exception.unwrap().name, "Root");
        assert_eq!(
            result
                .causes
                .iter()
                .map(|cause| cause.error.name.as_str())
                .collect::<Vec<_>>(),
            ["Second", "Tenth"]
        );
        assert_eq!(result.revision.as_deref(), Some("rev"));
    }

    #[test]
    fn missing_indexes_do_not_turn_exception_events_into_causes() {
        let result = parse(&[span(
            "span",
            vec![
                event("Root", "", Some("digest-a")),
                event("Other", "0_1", None),
            ],
        )])
        .unwrap();
        assert!(result.causes.is_empty());
    }

    #[test]
    fn malformed_and_truncated_event_arrays_are_safe() {
        let mut value = span(
            "span",
            vec![
                event("Root", "0_0", Some("digest-a")),
                json!({"exception.type": 42}),
                event("Duplicate1", "0_1", None),
                event("Duplicate2", "0_1", None),
            ],
        );
        value.event_names.push("exception".into());
        value.event_names.push("log".into());
        let result = parse(&[value]).unwrap();
        assert!(result.causes.is_empty());
    }

    #[test]
    fn primary_selection_prefers_summary_span_but_never_guesses() {
        let first = span("span", vec![event("Selected", "0_0", Some("digest-a"))]);
        let other = span("other", vec![event("Other", "0_0", Some("digest-b"))]);
        assert_eq!(
            parse(&[first.clone(), other.clone()])
                .unwrap()
                .exception
                .unwrap()
                .name,
            "Selected"
        );
        let mut no_span = summary();
        no_span.span_id = None;
        assert!(parse_exception(
            &["digest-a".into(), "digest-b".into()],
            &no_span,
            &[first, other]
        )
        .is_err());
        assert!(parse(&[span(
            "span",
            vec![
                event("One", "0_0", Some("digest-a")),
                event("Two", "1_0", Some("digest-b"))
            ]
        )])
        .is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn selected_span_must_match_and_only_absent_id_allows_fallback() {
        let other = span("other", vec![event("Other", "0_0", Some("digest-a"))]);
        for spans in [
            vec![other.clone()],
            vec![span("span", vec![]), other.clone()],
        ] {
            let error = parse(&spans).err().expect("must not select another span");
            assert!(matches!(
                error.downcast_ref::<SelectionError>(),
                Some(SelectionError::MissingSelectedSpan)
            ));
        }
        let mut no_span = summary();
        no_span.span_id = None;
        let parsed = parse_exception(&["digest-a".into()], &no_span, &[other]).unwrap();
        assert_eq!(parsed.exception.unwrap().name, "Other");
    }

    #[test]
    fn diagnostic_reasons_are_distinct_and_do_not_expose_server_payloads() {
        let auth = CliError::AuthRejected {
            detail: Some("private payload".into()),
        };
        assert!(failure_reason(&auth.into()).contains("authentication rejected"));
        let schema = CliError::GraphQlRejected("private schema payload".into());
        assert_eq!(failure_reason(&schema.into()), "GraphQL query was rejected");
        assert_eq!(
            failure_reason(&CliError::UnexpectedResponse.into()),
            "server returned an unexpected response"
        );
        assert_eq!(
            failure_reason(&SelectionError::MissingSelectedSpan.into()),
            "the selected span has no matching exception"
        );
        assert_eq!(
            failure_reason(&anyhow::anyhow!("private internal payload")),
            "unexpected internal error"
        );
    }

    #[test]
    fn backtrace_uses_first_app_frame_and_existing_format() {
        let lines = |value| serde_json::from_value::<Vec<BacktraceLine>>(value).unwrap();
        assert_eq!(
            first_app_line(&lines(json!([
                {"type": "line", "original": "library"},
                {"type": "app", "original": "  ", "path": " app/report.rb ", "line": " 42 "},
                {"type": "app", "original": "later"}
            ])))
            .as_deref(),
            Some("app/report.rb:42")
        );
        assert_eq!(
            first_app_line(&lines(
                json!([{"type": "app", "original": " original ", "path": "ignored"}])
            ))
            .as_deref(),
            Some("original")
        );
        assert!(first_app_line(&lines(json!([{"type": "line", "original": "library"}]))).is_none());
    }

    async fn mount_trace(server: &MockServer, spans: Vec<TraceSpan>) {
        Mock::given(method("POST"))
            .and(path("/api/v2/tracing/traces/errors"))
            .and(header("authorization", "Bearer tok"))
            .and(header("x-appsignal-client", "appsignal-cli"))
            .and(header(
                "x-appsignal-client-version",
                env!("CARGO_PKG_VERSION"),
            ))
            .and(body_partial_json(json!({
                "site_ids": ["app"], "digests": ["digest-a", "digest-b"],
                "pagination": {"per_page": 1, "order": "DESC", "cursor": {"time": null}}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([summary()])))
            .expect(1)
            .mount(server)
            .await;
        Mock::given(method("POST")).and(path("/api/v2/tracing/trace/error"))
            .and(header("authorization", "Bearer tok"))
            .and(body_partial_json(json!({"site_ids": ["app"], "digests": ["digest-a", "digest-b"], "trace_id": "trace"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(spans)))
            .expect(1).mount(server).await;
    }

    #[tokio::test]
    async fn enrichment_deduplicates_backtraces_and_respects_separate_endpoints() {
        let rest = MockServer::start().await;
        let graphql = MockServer::start().await;
        let mut cause2 = event("Second", "0_2", None);
        cause2["appsignal.stacktrace_id"] = json!("stack-First");
        mount_trace(
            &rest,
            vec![span(
                "span",
                vec![
                    event("Root", "0_0", Some("digest-b")),
                    event("First", "0_1", None),
                    cause2,
                ],
            )],
        )
        .await;
        Mock::given(method("POST")).and(path("/graphql"))
            .and(header("authorization", "Bearer tok"))
            .and(header("x-appsignal-client", "appsignal-cli"))
            .and(body_partial_json(json!({"variables": {"appId": "app", "id": "stack-First", "revision": "rev"}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"app": {"backtrace": [null, {"type": "app", "path": "app.rb", "line": "4"}]}}
            }))).expect(1).mount(&graphql).await;
        let client = AppSignalClient::with_auth_endpoints(
            crate::config::AuthMethod::OAuth {
                access_token: "tok".into(),
                refresh_token: None,
                expires_at: None,
            },
            Some(&graphql.uri()),
            Some(&rest.uri()),
        );
        let result = fetch(&client, "app", &incident()).await;
        assert!(result.warning.is_none());
        assert_eq!(result.exception.unwrap().name, "Root");
        assert_eq!(result.causes.len(), 2);
        assert!(result
            .causes
            .iter()
            .all(|cause| cause.error.first_line.as_deref() == Some("app.rb:4")));
        let requests = graphql.received_requests().await.unwrap();
        assert!(!requests[0].body_json::<Value>().unwrap()["query"]
            .as_str()
            .unwrap()
            .contains("sample"));
    }

    #[tokio::test]
    async fn failed_or_missing_backtraces_keep_causes() {
        let server = MockServer::start().await;
        mount_trace(
            &server,
            vec![span(
                "span",
                vec![
                    event("Root", "0_0", Some("digest-a")),
                    event("First", "0_1", None),
                    event("Second", "0_2", None),
                ],
            )],
        )
        .await;
        Mock::given(path("/graphql"))
            .and(body_partial_json(
                json!({"variables": {"id": "stack-First"}}),
            ))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(path("/graphql"))
            .and(body_partial_json(
                json!({"variables": {"id": "stack-Second"}}),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"app": {"backtrace": null}}})),
            )
            .mount(&server)
            .await;
        let result = fetch(
            &AppSignalClient::with_endpoint("tok", &server.uri()),
            "app",
            &incident(),
        )
        .await;
        assert!(result
            .warning
            .as_deref()
            .unwrap()
            .contains("server is unavailable"));
        assert_eq!(result.causes.len(), 2);
        assert!(result
            .causes
            .iter()
            .all(|cause| cause.error.first_line.is_none()));
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_preserves_completed_backtraces() {
        let mut parsed = parse(&[span(
            "span",
            vec![
                event("Root", "0_0", Some("digest-a")),
                event("Fast", "0_1", None),
                event("Slow", "0_2", None),
            ],
        )])
        .unwrap();
        let deadline = Duration::from_secs(10);
        let start = tokio::time::Instant::now();
        let response = tokio::time::timeout(
            deadline,
            enrich_backtraces(&mut parsed.causes, |id| async move {
                if id == "stack-Fast" {
                    Ok(
                        serde_json::from_value(json!([{"type": "app", "original": "fast.rb:1"}]))
                            .unwrap(),
                    )
                } else {
                    std::future::pending().await
                }
            }),
        )
        .await;
        assert!(response.is_err());
        assert_eq!(tokio::time::Instant::now() - start, deadline);
        assert_eq!(
            parsed.causes[0].error.first_line.as_deref(),
            Some("fast.rb:1")
        );
        assert!(parsed.causes[1].error.first_line.is_none());
    }

    #[tokio::test]
    async fn no_digests_or_retained_traces_is_normal() {
        let server = MockServer::start().await;
        let client = AppSignalClient::with_endpoint("tok", &server.uri());
        let mut no_digests = incident();
        if let Incident::ExceptionIncident { digests, .. } = &mut no_digests {
            *digests = Some(vec![" ".into()]);
        }
        assert!(fetch(&client, "app", &no_digests).await.warning.is_none());
        assert!(server.received_requests().await.unwrap().is_empty());
        Mock::given(path("/api/v2/tracing/traces/errors"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        let result = fetch(&client, "app", &incident()).await;
        assert!(result.warning.is_none());
        assert!(result.exception.is_none());
    }

    #[tokio::test]
    async fn selected_span_mismatch_warns_without_enriching_from_another_span() {
        let server = MockServer::start().await;
        mount_trace(
            &server,
            vec![span("other", vec![event("Other", "0_0", Some("digest-a"))])],
        )
        .await;
        let result = fetch(
            &AppSignalClient::with_endpoint("tok", &server.uri()),
            "app",
            &incident(),
        )
        .await;
        assert!(result.exception.is_none());
        assert!(result.causes.is_empty());
        assert!(result
            .warning
            .as_deref()
            .unwrap()
            .contains("selected span has no matching exception"));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn backtrace_pool_refills_and_preserves_cause_order() {
        let mut events = vec![event("Root", "0_0", Some("digest-a"))];
        for index in 1..=6 {
            events.push(event(&format!("Cause{index}"), &format!("0_{index}"), None));
        }
        let mut parsed = parse(&[span("span", events)]).unwrap();
        let warning = enrich_backtraces(&mut parsed.causes, |id| async move {
            // Complete requests in a different order than the cause chain.
            let index: u64 = id.strip_prefix("stack-Cause").unwrap().parse().unwrap();
            tokio::time::sleep(Duration::from_secs(7 - index)).await;
            Ok(serde_json::from_value(json!([{"type": "app", "original": id}])).unwrap())
        })
        .await;
        assert!(warning.is_none());
        for (index, cause) in parsed.causes.iter().enumerate() {
            assert_eq!(cause.error.name, format!("Cause{}", index + 1));
            assert_eq!(
                cause.error.first_line,
                Some(format!("stack-Cause{}", index + 1))
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn backtrace_requests_are_limited_to_four_and_cancelled_at_deadline() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        struct ActiveRequest {
            active: Arc<AtomicUsize>,
            cancelled: Arc<tokio::sync::Semaphore>,
        }
        impl Drop for ActiveRequest {
            fn drop(&mut self) {
                self.active.fetch_sub(1, Ordering::SeqCst);
                self.cancelled.add_permits(1);
            }
        }

        let mut events = vec![event("Root", "0_0", Some("digest-a"))];
        for index in 1..=6 {
            events.push(event(&format!("Cause{index}"), &format!("0_{index}"), None));
        }
        let mut parsed = parse(&[span("span", events)]).unwrap();
        let started = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let cancelled = Arc::new(tokio::sync::Semaphore::new(0));
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            enrich_backtraces(&mut parsed.causes, |_| {
                let started = started.clone();
                let active = active.clone();
                let cancelled = cancelled.clone();
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    active.fetch_add(1, Ordering::SeqCst);
                    let _guard = ActiveRequest { active, cancelled };
                    std::future::pending().await
                }
            }),
        )
        .await;
        assert!(response.is_err());
        assert_eq!(started.load(Ordering::SeqCst), 4);
        // Wait for all four futures to be dropped, without scheduler assumptions.
        tokio::time::timeout(Duration::from_secs(1), cancelled.acquire_many(4))
            .await
            .unwrap()
            .unwrap()
            .forget();
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(started.load(Ordering::SeqCst), 4);
    }
}
