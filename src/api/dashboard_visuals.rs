//! Dashboard visual inputs and the app-scoped GraphQL operations that use them.
use super::{AppSignalClient, Dashboard};
use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Value};

use crate::error::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum VisualType {
    Timeseries,
    Number,
}

/// Unlike Option, a patch must distinguish omission from explicitly clearing a value.
#[derive(Debug, Default, Clone)]
pub enum PatchField<T> {
    Value(T),
    Null,
    #[default]
    Missing,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchField<T> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

impl<T> PatchField<T> {
    fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<T: Serialize> Serialize for PatchField<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Null | Self::Missing => serializer.serialize_none(),
        }
    }
}

macro_rules! wire_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Deserialize, Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name { $($variant),+ }
    };
}
wire_enum!(Display {
    Line,
    Area,
    AreaRelative
});
wire_enum!(MetricField {
    Mean,
    P90,
    P95,
    Count,
    Gauge,
    Counter
});
wire_enum!(Aggregate {
    Max,
    Min,
    Avg,
    Sum,
    First,
    Last
});

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VisualFormat {
    Number,
    Percent,
    Duration,
    Throughput,
    Size,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatInput {
    Bit,
    Byte,
    Kilobit,
    Kilobyte,
    Megabyte,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    pub key: String,
    pub value: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetricSelection {
    pub field: MetricField,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimeseriesMetric {
    pub name: String,
    pub fields: Vec<MetricSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumberMetric {
    pub name: String,
    pub field: MetricField,
    pub aggregate: Aggregate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

macro_rules! visual_input {
    ($($field:ident: $ty:ty),+ $(,)?) => {
        #[derive(Debug, Default, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct VisualInput {
            $(#[serde(default, skip_serializing_if = "PatchField::is_missing")]
            pub $field: PatchField<$ty>),+
        }
    };
}
visual_input! {
    title: String,
    description: String,
    format: VisualFormat,
    format_input: FormatInput,
    layout: Layout,
    line_label: String,
    display: Display,
    draw_null_as_zero: bool,
    min_y_axis: f64,
    metrics: Vec<TimeseriesMetric>,
    metric: NumberMetric,
}

impl VisualInput {
    pub fn parse(reader: impl std::io::Read) -> Result<Self> {
        let value: Value = serde_json::from_reader(reader)
            .map_err(|error| CliError::msg(format!("Invalid chart JSON: {error}")))?;
        if !value.is_object() {
            return Err(CliError::msg("Chart JSON must be an object.").into());
        }
        serde_json::from_value(value)
            .map_err(|error| CliError::msg(format!("Invalid chart JSON: {error}")).into())
    }

    pub fn validate(&self, kind: VisualType, creating: bool) -> Result<()> {
        let invalid = |message: &str| Err(CliError::msg(message.to_owned()).into());
        match &self.title {
            PatchField::Null => return invalid("Chart title cannot be null."),
            PatchField::Value(title) if title.trim().is_empty() => {
                return invalid("Chart title cannot be empty.")
            }
            PatchField::Missing if creating => return invalid("Chart creation requires a title."),
            _ => {}
        }
        if serde_json::to_value(self)?
            .as_object()
            .is_some_and(|o| o.is_empty())
        {
            return invalid("Chart changes cannot be empty.");
        }
        match kind {
            VisualType::Timeseries if !self.metric.is_missing() => return invalid("Timeseries charts use metrics, not metric."),
            VisualType::Number if !self.metrics.is_missing() || !self.line_label.is_missing()
                || !self.display.is_missing() || !self.draw_null_as_zero.is_missing()
                || !self.min_y_axis.is_missing() => return invalid("Number charts do not accept timeseries settings (metrics, lineLabel, display, drawNullAsZero, minYAxis)."),
            _ => {}
        }
        if matches!(self.display, PatchField::Null)
            || matches!(self.draw_null_as_zero, PatchField::Null)
            || matches!(self.metrics, PatchField::Null)
        {
            return invalid("display, drawNullAsZero and metrics cannot be null; use an empty metrics array to clear metrics.");
        }
        if let PatchField::Value(metrics) = &self.metrics {
            for metric in metrics {
                validate_metric(&metric.name, metric.tags.as_deref())?;
            }
        }
        if let PatchField::Value(metric) = &self.metric {
            validate_metric(&metric.name, metric.tags.as_deref())?;
        }
        Ok(())
    }
}

fn validate_metric(name: &str, tags: Option<&[Tag]>) -> Result<()> {
    if name.trim().is_empty() {
        return Err(CliError::msg("Metric name cannot be empty.").into());
    }
    if tags.is_some_and(|tags| {
        tags.iter()
            .any(|tag| tag.key.trim().is_empty() || tag.value.trim().is_empty())
    }) {
        return Err(CliError::msg("Metric tag keys and values cannot be empty.").into());
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualCommon {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub format: Option<VisualFormat>,
    pub format_input: Option<FormatInput>,
    pub layout: Option<Layout>,
}

// Response fields are nullable even though mutation inputs require values.
#[derive(Debug, Deserialize, Serialize)]
pub struct MetricSelectionDetail {
    pub field: Option<MetricField>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TagDetail {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TimeseriesMetricDetail {
    pub name: String,
    pub fields: Option<Vec<MetricSelectionDetail>>,
    pub tags: Option<Vec<TagDetail>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NumberMetricDetail {
    pub name: String,
    pub field: Option<MetricField>,
    pub aggregate: Option<Aggregate>,
    pub tags: Option<Vec<TagDetail>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "__typename")]
pub enum Visual {
    VisualTimeseries {
        #[serde(flatten)]
        common: VisualCommon,
        #[serde(rename = "lineLabel")]
        line_label: Option<String>,
        display: Option<Display>,
        #[serde(rename = "drawNullAsZero")]
        draw_null_as_zero: Option<bool>,
        #[serde(rename = "minYAxis")]
        min_y_axis: Option<f64>,
        metrics: Vec<TimeseriesMetricDetail>,
    },
    VisualNumber {
        #[serde(flatten)]
        common: VisualCommon,
        metric: Option<NumberMetricDetail>,
    },
}
impl Visual {
    pub fn common(&self) -> &VisualCommon {
        match self {
            Self::VisualTimeseries { common, .. } | Self::VisualNumber { common, .. } => common,
        }
    }
    pub fn kind(&self) -> VisualType {
        match self {
            Self::VisualTimeseries { .. } => VisualType::Timeseries,
            Self::VisualNumber { .. } => VisualType::Number,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DashboardDetail {
    #[serde(flatten)]
    pub dashboard: Dashboard,
    pub visuals: Vec<Visual>,
}

const DETAIL_SELECTION: &str = r#"
    id title description label source createdAt updatedAt
    visuals {
        __typename
        ... on VisualTimeseries {
            id title description format formatInput layout { x y w h }
            lineLabel display drawNullAsZero minYAxis
            metrics { name fields { field } tags { key value } }
        }
        ... on VisualNumber {
            id title description format formatInput layout { x y w h }
            metric { name field aggregate tags { key value } }
        }
    }
"#;

impl AppSignalClient {
    pub async fn get_dashboard_detail(
        &self,
        app_id: &str,
        dashboard_id: &str,
    ) -> Result<DashboardDetail> {
        #[derive(Deserialize)]
        struct Data {
            app: Option<App>,
        }
        #[derive(Deserialize)]
        struct App {
            dashboard: Option<DashboardDetail>,
        }
        let query = format!("query DashboardDetail($appId: String!, $id: String!) {{ app(id: $appId) {{ dashboard(id: $id) {{ {DETAIL_SELECTION} }} }} }}");
        let data: Data = self
            .graphql(&query, json!({"appId": app_id, "id": dashboard_id}))
            .await?;
        data.app.and_then(|app| app.dashboard).with_context(|| {
            CliError::msg(format!(
                "Dashboard {dashboard_id} was not found in app {app_id}."
            ))
        })
    }

    pub async fn create_dashboard_visual(
        &self,
        app_id: &str,
        dashboard_id: &str,
        kind: VisualType,
        input: VisualInput,
    ) -> Result<DashboardDetail> {
        input.validate(kind, true)?;
        self.mutate_visual(app_id, dashboard_id, kind, input, true)
            .await
    }

    pub async fn update_dashboard_visual(
        &self,
        app_id: &str,
        dashboard_id: &str,
        visual_id: &str,
        mut input: VisualInput,
    ) -> Result<DashboardDetail> {
        let dashboard = self.get_dashboard_detail(app_id, dashboard_id).await?;
        let visual = dashboard
            .visuals
            .iter()
            .find(|visual| visual.common().id == visual_id)
            .with_context(|| {
                CliError::msg(format!(
                    "Chart {visual_id} was not found in dashboard {dashboard_id}."
                ))
            })?;
        input.validate(visual.kind(), false)?;
        if input.title.is_missing() {
            input.title = PatchField::Value(visual.common().title.clone());
        }
        self.mutate_visual(app_id, visual_id, visual.kind(), input, false)
            .await
    }

    async fn mutate_visual(
        &self,
        app_id: &str,
        id: &str,
        kind: VisualType,
        input: VisualInput,
        creating: bool,
    ) -> Result<DashboardDetail> {
        let (mutation, input_type) = match (creating, kind) {
            (true, VisualType::Timeseries) => ("createVisualTimeseries", "VisualTimeseriesInput"),
            (false, VisualType::Timeseries) => ("updateVisualTimeseries", "VisualTimeseriesInput"),
            (true, VisualType::Number) => ("createVisualNumber", "VisualNumberInput"),
            (false, VisualType::Number) => ("updateVisualNumber", "VisualNumberInput"),
        };
        let id_arg = if creating { "dashboardId" } else { "visualId" };
        let query = format!("mutation ManageVisual($appId: String!, $id: String!, $visual: {input_type}!) {{ dashboard: {mutation}(appId: $appId, {id_arg}: $id, visual: $visual) {{ {DETAIL_SELECTION} }} }}");
        #[derive(Deserialize)]
        struct Data {
            dashboard: Option<DashboardDetail>,
        }
        let data: Data = self
            .graphql(&query, json!({"appId": app_id, "id": id, "visual": input}))
            .await?;
        data.dashboard.context(CliError::msg(
            "The API did not return the updated dashboard.",
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub fn dashboard() -> Value {
        json!({"id":"dash-1", "title":"Overview", "source":"USER", "visuals":[
            {"__typename":"VisualTimeseries", "id":"line-1", "title":"Latency",
             "display":"LINE", "format":"duration", "layout":{"x":0,"y":0,"w":6,"h":4},
             "metrics":[{"name":"duration", "fields":[{"field":"MEAN"}], "tags":[{"key":"namespace","value":"web"}]}]},
            {"__typename":"VisualNumber", "id":"number-1", "title":"Requests",
             "metric":{"name":"requests", "field":"COUNT", "aggregate":"SUM"}}
        ]})
    }

    fn input(value: Value) -> VisualInput {
        VisualInput::parse(value.to_string().as_bytes()).unwrap()
    }

    async fn lookup(server: &MockServer, value: Value) {
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_partial_json(
                json!({"variables":{"appId":"app-1", "id":"dash-1"}}),
            ))
            .and(|req: &wiremock::Request| {
                req.body_json::<Value>().unwrap()["query"]
                    .as_str()
                    .unwrap()
                    .starts_with("query ")
            })
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data":{"app":{"dashboard":value}}})),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    #[test]
    fn parses_and_validates_both_types() {
        input(json!({"title":"Latency", "display":"AREA_RELATIVE", "metrics":[{"name":"duration","fields":[{"field":"P95"}]}]}))
            .validate(VisualType::Timeseries, true).unwrap();
        input(json!({"title":"Requests", "metric":{"name":"requests","field":"COUNT","aggregate":"SUM"}}))
            .validate(VisualType::Number, true).unwrap();
    }

    #[test]
    fn malformed_unknown_and_incomplete_fields_are_rejected() {
        for text in [
            "{",
            "[]",
            "null",
            r#"{"titel":"Oops"}"#,
            r#"{"display":"BAR"}"#,
            r#"{"layout":{"x":0}}"#,
            r#"{"layout":{"x":0,"y":0,"w":1.5,"h":1}}"#,
            r#"{"metrics":[{"name":"duration","fields":[{"field":"MEDIAN"}]}]}"#,
            r#"{"metric":{"name":"requests","field":"COUNT"}}"#,
            r#"{"metric":{"name":"requests","field":"COUNT","aggregate":"SUM","unknown":1}}"#,
        ] {
            let error = VisualInput::parse(text.as_bytes()).unwrap_err();
            assert!(error.downcast_ref::<CliError>().is_some(), "{text}");
        }
    }

    #[test]
    fn invalid_patches_and_type_mismatches_are_rejected() {
        for (value, kind) in [
            (json!({}), VisualType::Timeseries),
            (json!({"title":null}), VisualType::Number),
            (json!({"title":" "}), VisualType::Timeseries),
            (json!({"display":null}), VisualType::Timeseries),
            (json!({"metrics":null}), VisualType::Timeseries),
            (json!({"drawNullAsZero":null}), VisualType::Timeseries),
            (json!({"metric":null}), VisualType::Timeseries),
            (json!({"metrics":[]}), VisualType::Number),
            (json!({"display":"LINE"}), VisualType::Number),
            (
                json!({"metrics":[{"name":"","fields":[]}]}),
                VisualType::Timeseries,
            ),
        ] {
            assert!(
                input(value.clone()).validate(kind, false).is_err(),
                "{value}"
            );
        }
        assert!(input(json!({"description":"No title"}))
            .validate(VisualType::Number, true)
            .is_err());
    }

    #[test]
    fn patch_serialization_preserves_null_false_zero_and_omission() {
        let value = json!({"description":null,"layout":null,"drawNullAsZero":false,"minYAxis":0.0,"metrics":[]});
        let patch = input(value.clone());
        patch.validate(VisualType::Timeseries, false).unwrap();
        assert_eq!(serde_json::to_value(patch).unwrap(), value);
        let patch = input(json!({"metric":null}));
        patch.validate(VisualType::Number, false).unwrap();
        assert_eq!(serde_json::to_value(patch).unwrap(), json!({"metric":null}));
    }

    #[test]
    fn response_accepts_nullable_metric_fields() {
        let mut value = dashboard();
        value["visuals"][0]["metrics"][0]["fields"] = Value::Null;
        value["visuals"][1]["metric"]["field"] = Value::Null;
        value["visuals"][1]["metric"]["aggregate"] = Value::Null;
        let detail: DashboardDetail = serde_json::from_value(value).unwrap();
        assert_eq!(detail.visuals[0].common().title, "Latency");
        assert_eq!(detail.visuals[1].kind(), VisualType::Number);
    }

    fn dashboard_with_nested_nulls() -> Value {
        let mut value = dashboard();
        value["visuals"][0]["metrics"][0]["fields"] = json!([{"field": null}]);
        value["visuals"][0]["metrics"][0]["tags"] = json!([{"key": "namespace", "value": null}]);
        value["visuals"][1]["metric"]["tags"] = json!([{"key": "hostname", "value": null}]);
        value
    }

    #[test]
    fn response_preserves_nested_null_metric_fields_and_tag_values() {
        let detail: DashboardDetail =
            serde_json::from_value(dashboard_with_nested_nulls()).unwrap();
        let result = serde_json::to_value(detail).unwrap();
        assert_eq!(
            result["visuals"][0]["metrics"][0]["fields"],
            json!([{"field": null}])
        );
        assert_eq!(
            result["visuals"][0]["metrics"][0]["tags"],
            json!([{"key": "namespace", "value": null}])
        );
        assert_eq!(
            result["visuals"][1]["metric"]["tags"],
            json!([{"key": "hostname", "value": null}])
        );
    }

    #[test]
    fn inputs_reject_null_metric_fields_and_tag_values() {
        for value in [
            json!({"title":"Latency", "metrics":[{"name":"duration", "fields":[{"field":null}]}]}),
            json!({"title":"Latency", "metrics":[{"name":"duration", "fields":[{"field":"MEAN"}], "tags":[{"key":"namespace", "value":null}]}]}),
            json!({"title":"Requests", "metric":{"name":"requests", "field":null, "aggregate":"SUM"}}),
            json!({"title":"Requests", "metric":{"name":"requests", "field":"COUNT", "aggregate":"SUM", "tags":[{"key":"hostname", "value":null}]}}),
        ] {
            assert!(
                VisualInput::parse(value.to_string().as_bytes()).is_err(),
                "{value}"
            );
        }
    }

    #[tokio::test]
    async fn invalid_creation_does_not_call_the_api() {
        let server = MockServer::start().await;
        let client = AppSignalClient::with_endpoint("tok", &server.uri());
        for definition in [
            json!({"title":""}),
            json!({"title":"Requests", "metric":{"name":"requests","field":"COUNT","aggregate":"SUM","tags":[{"key":"namespace","value":""}]}}),
        ] {
            assert!(client
                .create_dashboard_visual("app-1", "dash-1", VisualType::Number, input(definition))
                .await
                .is_err());
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn creates_both_chart_types_with_oauth_and_cli_headers() {
        for (kind, mutation, definition) in [
            (
                VisualType::Timeseries,
                "createVisualTimeseries",
                json!({"title":"Latency","metrics":[{"name":"duration","fields":[{"field":"MEAN"}]}]}),
            ),
            (
                VisualType::Number,
                "createVisualNumber",
                json!({"title":"Requests","metric":{"name":"requests","field":"COUNT","aggregate":"SUM"}}),
            ),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/graphql"))
                .and(header("Authorization", "Bearer tok"))
                .and(header("X-AppSignal-Client", "appsignal-cli"))
                .and(header(
                    "X-AppSignal-Client-Version",
                    env!("CARGO_PKG_VERSION"),
                ))
                .and(header(
                    "User-Agent",
                    crate::client_headers::USER_AGENT_VALUE,
                ))
                .and(body_partial_json(
                    json!({"variables":{"appId":"app-1","id":"dash-1","visual":definition}}),
                ))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"data":{"dashboard":dashboard_with_nested_nulls()}})),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = AppSignalClient::with_endpoint("tok", &server.uri());
            let result = client
                .create_dashboard_visual("app-1", "dash-1", kind, input(definition))
                .await
                .unwrap();
            assert_eq!(result.visuals.len(), 2);
            let requests = server.received_requests().await.unwrap();
            let body: Value = requests[0].body_json().unwrap();
            assert!(body["query"].as_str().unwrap().contains(mutation));
            assert!(body["query"].as_str().unwrap().contains("dashboardId: $id"));
        }
    }

    #[tokio::test]
    async fn partial_updates_send_only_changed_fields_and_required_title() {
        for (id, mutation, patch, expected) in [
            (
                "line-1",
                "updateVisualTimeseries",
                json!({"title":"Renamed"}),
                json!({"title":"Renamed"}),
            ),
            (
                "number-1",
                "updateVisualNumber",
                json!({"description":null}),
                json!({"title":"Requests","description":null}),
            ),
            (
                "line-1",
                "updateVisualTimeseries",
                json!({"metrics":[{"name":"new","fields":[{"field":"P90"}]}],"layout":{"x":1,"y":2,"w":3,"h":4}}),
                json!({"title":"Latency","metrics":[{"name":"new","fields":[{"field":"P90"}]}],"layout":{"x":1,"y":2,"w":3,"h":4}}),
            ),
        ] {
            let server = MockServer::start().await;
            lookup(&server, dashboard_with_nested_nulls()).await;
            Mock::given(method("POST"))
                .and(body_partial_json(json!({"variables":{"id":id}})))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"data":{"dashboard":dashboard_with_nested_nulls()}})),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = AppSignalClient::with_endpoint("tok", &server.uri());
            client
                .update_dashboard_visual("app-1", "dash-1", id, input(patch))
                .await
                .unwrap();
            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 2);
            let body: Value = requests[1].body_json().unwrap();
            assert_eq!(body["variables"]["visual"], expected);
            assert_eq!(body["variables"]["appId"], "app-1");
            assert!(body["query"].as_str().unwrap().contains(mutation));
            assert!(body["query"].as_str().unwrap().contains("visualId: $id"));
        }
    }

    #[tokio::test]
    async fn missing_chart_or_wrong_type_never_mutates() {
        for (id, patch) in [
            ("missing", json!({"title":"New"})),
            ("number-1", json!({"display":"LINE"})),
            ("line-1", json!({})),
        ] {
            let server = MockServer::start().await;
            lookup(&server, dashboard_with_nested_nulls()).await;
            let client = AppSignalClient::with_endpoint("tok", &server.uri());
            let error = client
                .update_dashboard_visual("app-1", "dash-1", id, input(patch))
                .await
                .unwrap_err();
            assert!(error.downcast_ref::<CliError>().is_some());
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn missing_dashboard_is_a_user_error() {
        let server = MockServer::start().await;
        lookup(&server, Value::Null).await;
        let client = AppSignalClient::with_endpoint("tok", &server.uri());
        let error = client
            .get_dashboard_detail("app-1", "dash-1")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Dashboard dash-1 was not found"));
    }

    #[tokio::test]
    async fn mutation_errors_are_reported_without_retry() {
        for response in [
            ResponseTemplate::new(400)
                .set_body_json(json!({"errors":[{"message":"Invalid chart"}]})),
            ResponseTemplate::new(200)
                .set_body_json(json!({"errors":[{"message":"Invalid chart"}]})),
            ResponseTemplate::new(200).set_body_json(json!({"data":{"dashboard":null}})),
            ResponseTemplate::new(403),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(response)
                .expect(1)
                .mount(&server)
                .await;
            let client = AppSignalClient::with_endpoint("tok", &server.uri());
            let error = client
                .create_dashboard_visual(
                    "app-1",
                    "dash-1",
                    VisualType::Timeseries,
                    input(json!({"title":"Latency"})),
                )
                .await
                .unwrap_err();
            assert!(error.downcast_ref::<CliError>().is_some());
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
