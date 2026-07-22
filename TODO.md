# MCP Feature Parity Tracking

This document tracks feature parity between `appsignal-cli` and the AppSignal MCP server.

Status legend:
- [ ] Not started
- [~] Partial parity
- [x] Complete
- [n/a] Not a CLI parity target

Key implementation difference:
- The MCP server has access to internal APIs and direct application data.
- The CLI currently uses the public GraphQL API, and may need REST endpoints for metrics and other missing features.

REST migration tracking:
- See `rest-todo.md` for the current GraphQL-to-REST migration inventory and implementation order.

## Current Summary

Implemented in the CLI today:
- Applications and org discovery
- App resources: users, notifiers, namespaces, dashboards
- Incident listing, incident detail, incident update, incident notes
- Log search, log tail, log views, log sources
- Trigger listing, creation, update, and archiving

Still missing or incomplete:
- Metrics discovery and querying
- Dashboard visual management
- Trace inspection
- Log line action management
- Deploy marker access
- Full performance overview parity
- Some incident filter parity

---

## Applications

### `get_applications`
- [x] `apps list`
- [x] `apps find --name <name> [--environment <env>]`
- [x] `apps info --app-id <id>`

Notes:
- The CLI requires org resolution because it works through the public API.
- The org slug is persisted after `apps list` or `apps set-org --org ...`.

### `get_app_resources`
- [~] `apps resources <resource>`
- [x] `apps resources users`
- [x] `apps resources notifiers`
- [x] `apps resources namespaces`
- [x] `apps resources dashboards`
- [x] `apps resources deploy-markers` covers `deploy_markers`
- [x] `apps resources all` provides the combined view
- [x] `logs sources` covers `log_sources`
- [x] `logs views` covers `log_views`
- [n/a] Do not expose `log_line_actions` directly; use `logs metrics` and `logs triggers`

Notes:
- The MCP tool returns more resource sections than the current CLI does.
- The CLI now prefers one resource per call, with `all` as the explicit batch mode.
- The remaining work is extending coverage.

---

## Incidents

### `get_exception_incidents`
- [~] `incidents list-exceptions`

Covered today:
- `--app` / `--environment` or `--app-id`
- `--namespaces`
- `--action`
- `--state` (single state)
- `--query`
- `--limit` / `--offset`

Still missing:
- [ ] `start`
- [ ] `end`
- [ ] `revision`
- [ ] multiple states in a single request

Notes:
- The current GraphQL query only supports a single incident state enum.
- Time-range and deploy-scoped filters may require different API support.

### `get_anomaly_incidents`
- [~] `incidents list-anomalies`

Covered today:
- app selection
- pagination via `--limit` / `--offset`
- `--state`

Still missing:
- [ ] `trigger_id`
- [ ] anomaly-specific states such as `warmup`, `cooldown`, and `archived`

### `get_incident`
- [x] `incidents show --number <N>`

Notes:
- The CLI incident detail query already handles multiple incident union types.

### `update_incidents`
- [~] `incidents update --number <N>`

Covered today:
- `--state`
- `--severity`
- `--assign`
- `--assign-me`
- `--unassign`
- `--description`
- bulk state updates by explicit incident number

Still missing:
- [ ] bulk severity, assignee, or description updates

Notes:
- The CLI resolves assignee names case-insensitively and merges them with the current incident assignee list.

### `create_incident_note`
- [x] `incidents add-note --number <N> --content "..."`
- [x] `incidents list-notes --number <N>`
- [x] `incidents update-note --number <N> --id <ID> --content "..."`
- [x] `incidents delete-note --number <N> --id <ID>`

---

## Performance And Traces

### `get_performance`
- [~] `incidents list-performance`

Covered today:
- app selection
- `--namespaces`
- `--action`
- `--state`
- `--query`
- pagination via `--limit` / `--offset`

Still missing:
- [ ] `start`
- [ ] `end`
- [ ] `revision`
- [ ] OTel action ranking output
- [ ] richer performance overview beyond incident listing

### `get_traces`
- [~] `samples` / `traces`

Covered today:
- [x] list samples/traces for a performance action with `samples list`
- [x] fetch samples/traces from a performance incident with `samples incident`
- [x] fetch error traces from an exception incident with `samples incident`
- [x] list error traces for an exception digest with `samples errors`
- [x] inspect a trace tree by trace ID with `samples show`
- [x] inspect a span by span ID with `samples show --span-id`
- [x] inspect an error trace tree or span with `samples show-error`
- [x] inspect performance or exception traces from an incident number with `samples show-incident`
- [x] cursor pagination with `--page-all`

---

## Logs

### `get_log_lines`
- [x] `logs search`

Covered today:
- `query`
- `source_ids`
- `start` / `end`
- `limit`
- `order`
- JSON output for machine-readable use

CLI extras:
- `logs tail`
- `--view` support that resolves saved log views by name or ID
- `--page-all` to auto-paginate beyond the 100-line API limit

---

## Triggers

### `get_triggers`
- [x] `triggers list`

### `manage_trigger`
- [x] `triggers create`
- [x] `triggers update`

### `archive_trigger`
- [x] `triggers archive`

---

## Metrics

### `discover_metrics`
- [ ] Not implemented

### `get_metric_names`
- [ ] Not implemented

### `get_metric_tags`
- [ ] Not implemented

### `get_metrics_list`
- [ ] Not implemented

### `get_metrics_timeseries`
- [ ] Not implemented

Likely implementation work:
- [ ] Add `metrics discover`
- [ ] Add `metrics names`
- [ ] Add `metrics tags`
- [ ] Add `metrics list`
- [ ] Add `metrics timeseries`

Notes:
- Metrics parity likely requires REST endpoints under `/api/v2/metrics/...`.
- The MCP server also has category-level discovery behavior that may rely on server-side config unavailable to the CLI.

---

## Dashboards

### `manage_dashboard`
- [x] `dashboards create`
- [x] `dashboards update`

### `create_dashboard_visual`
- [ ] Not implemented

### `update_dashboard_visual`
- [ ] Not implemented

Likely implementation work:
- [x] Add `dashboards create`
- [x] Add `dashboards update`
- [ ] Add `dashboards add-visual`
- [ ] Add `dashboards update-visual`

---

## Log Line Actions

### `manage_log_line_action`
- [~] Public CLI UX is split by product concept instead of exposing raw actions

Implemented today:
- [x] `logs metrics list`
- [x] `logs metrics create`
- [x] `logs metrics update`
- [x] `logs metrics delete`
- [x] `logs triggers list`
- [x] `logs triggers create`
- [x] `logs triggers update`
- [x] `logs triggers delete`

Still missing / undecided:
- [ ] Decide whether log filters need a public CLI surface

### `delete_log_line_action`
- [~] Covered indirectly by `logs metrics delete` and `logs triggers delete`

### `reorder_log_line_actions`
- [n/a] Internal ingestion-order concept, not part of the public CLI UX for now

Likely implementation work:
- [x] Add `logs metrics list`
- [x] Add `logs metrics create`
- [x] Add `logs metrics update`
- [x] Add `logs metrics delete`
- [x] Add `logs triggers list`
- [x] Add `logs triggers create`
- [x] Add `logs triggers update`
- [x] Add `logs triggers delete`
- [ ] Decide whether to add public `logs filters ...` commands

---

## Not Applicable

### `get_more_tools`
- [n/a] Internal MCP capability-discovery tool, not a CLI feature target

---

## Suggested Implementation Order

### Phase 1: Finish Core Read/Write Gaps
1. Add bulk incident updates
2. Add missing incident filters where the public API allows them

### Phase 2: Triggers And Metrics
1. Add `triggers list`
2. Add metrics name and tag discovery
3. Add metrics timeseries and aggregated list queries
4. Add broader metrics discovery if the underlying API supports it cleanly
5. Evaluate whether log filters need a public CLI surface

### Phase 3: Dashboards And Log Ingestion Controls
1. Add dashboard create/update
2. Add dashboard visual create/update
3. Add log filter CLI support if product requirements justify it

### Phase 4: Traces And Full Performance Parity
1. Add richer performance overview behavior
2. Add exception/performance timeframe and deploy filters if an API path exists

---

## Research Notes

- The public GraphQL API already covers more than the previous TODO suggested; this file was stale.
- Metrics likely require REST support in the CLI in addition to the existing GraphQL client.
- Trigger, dashboard, and log rule parity depends on whether the public API exposes the needed queries and mutations.
- Anomaly state handling may differ from exception/performance incident state enums, so state support should be verified before designing CLI flags.
