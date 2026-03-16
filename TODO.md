# MCP Feature Parity Tracking

This document tracks progress toward feature parity between the `appsignal-cli` and the AppSignal MCP server (`devenv/appsignal-server/app/mcp/tools/`).

**Key difference**: The MCP server uses direct MongoDB (Mongoid) access, while the CLI uses the public GraphQL API. Some MCP features may require discovering GraphQL mutations or using the REST API (`/api/[app_id]/...`).

## Status Legend

- [ ] Not started
- [~] Partially implemented
- [x] Complete

---

## Applications

### `get_applications`
- [x] **CLI**: `apps list --org <slug>`
- [x] **CLI**: `apps find --name <name> [--environment <env>]`
- [x] **CLI**: `apps orgs`
- [x] **CLI**: `apps info --app-id <id>`

**Notes**: CLI requires org slug (which the MCP server doesn't need since it has direct DB access). The org slug is persisted in config after first use.

---

## Incidents — Read

### `get_exception_incidents`
- [x] **CLI**: `incidents list-exceptions`

**MCP parameters**:
| Parameter | MCP | CLI | Notes |
|---|---|---|---|
| `app_name` | required | `--app` | Case-insensitive in CLI |
| `app_environment` | required | `--environment` | Case-insensitive in CLI |
| `namespaces` | comma-separated string | `--namespaces` | Done |
| `start` | ISO 8601 | Not available | GraphQL API does not expose start/end filters |
| `end` | ISO 8601 | Not available | GraphQL API does not expose start/end filters |
| `states` | comma-separated string | `--state` (single) | GraphQL only accepts single state enum |
| `page` | integer (1-based) | `--offset` | Different pagination model |
| `query` | string | `--query` | Text search for exception name/message — Done |
| `actionName` | string | `--action` | Filter by action — Done |

**Not possible via GraphQL**:
- `--start` / `--end` time range filters (MCP uses direct MongoDB, not available in GraphQL)
- Multiple states (GraphQL enum only accepts a single value)

### `get_anomaly_incidents`
- [x] **CLI**: `incidents list-anomalies`

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `state` | string | no | "open", "closed", "warmup", "cooldown", "archived" |
| `trigger_id` | string | no | Filter by trigger |
| `page` | integer | no | 1-based, 50 per page |

**Notes**: Implemented via `anomalyIncidents` GraphQL field. Supports `state`, `limit`, `offset`, `order`. Trigger ID filtering is not available via GraphQL (MCP uses direct MongoDB for that). Anomaly-specific fields (alertState, trigger summary, tags) are included in output.

### `get_incident`
- [x] **CLI**: `incidents show --number <N>`

**Notes**: CLI currently fetches all incident types (exception, performance, anomaly, log) via the `incident(incidentNumber)` GraphQL union query. The MCP server only supports exception and anomaly incidents in `get_incident`.

---

## Incidents — Write

### `update_incidents`
- [x] **CLI**: `incidents update`

Uses the `updateIncident` GraphQL mutation. Supports `--state`, `--severity`, `--assign` (comma-separated user IDs), and `--description`. Updates one incident at a time (by number).

**Notes**: The MCP server supports `unassign_users` separately; the CLI's `--assign` sets the full assignee list via `assigneeIds`. The GraphQL API also exposes `bulkUpdateIncidents` (by IDs, not numbers) but we use `updateIncident` (by number) for simplicity.

### `create_incident_note`
- [x] **CLI**: `incidents add-note`

Uses the `createIncidentNote` GraphQL mutation. Takes `--number`, `--content` (markdown supported), and the standard app resolution flags.

---

## Triggers (Anomaly Detection)

### `get_triggers`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `metric_name` | string | no | Filter by metric |
| `kind` | string | no | Filter by trigger kind |
| `tags` | array of {key, value} | no | Filter by tags |
| `page` | integer | no | 1-based, 50 per page |

**Returns**: Trigger ID, metric_name, kind, field, condition (operator + value), warmup/cooldown duration, tags, notifier count, dashboard_id, description.

**TODO**:
- [ ] Add `triggers list` command group
- [ ] Check if triggers are accessible via GraphQL (may need introspection)

---

## App Resources

### `get_app_resources`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `sections` | array of strings | no | "users", "notifiers", "namespaces", "dashboards" |

**Returns**:
- **users**: name, ID (for incident assignment)
- **notifiers**: name, ID, type (Slack, email, PagerDuty, etc.)
- **namespaces**: list of namespace strings
- **dashboards**: title, ID, description

**TODO**:
- [ ] Add `apps resources` subcommand
- [ ] This is critical for the `update_incidents` flow (need user IDs for assignment)
- [ ] Check GraphQL availability of users, notifiers, namespaces, dashboards

---

## Metrics — Read

### `discover_metrics`
- [ ] **CLI**: Not implemented

**MCP modes** (determined by `metric_subject` value):
1. No `metric_subject` → list all metric categories
2. `metric_subject` = category name → list metrics in category
3. `metric_subject` = `"dashboard:<id>"` → list metrics from a dashboard
4. `metric_subject` = `"custom_metrics"` → list custom (user-defined) metrics

**TODO**:
- [ ] Add `metrics discover` subcommand
- [ ] The MCP uses an internal YAML config (`AppsignalServer.metrics_config`) for categories — this won't be available via GraphQL. May need to use the REST API or skip category-level discovery.
- [ ] Custom metrics uses REST API: `GET /api/v2/metrics/names/:site_id`

### `get_metric_names`
- [ ] **CLI**: Not implemented

**MCP**: Calls `PublicApi::Metrics#names` → `GET /api/v2/metrics/names/:site_id`

**TODO**:
- [ ] Add `metrics names` subcommand
- [ ] Uses REST API, not GraphQL. Check if `metrics { list }` on the GraphQL `App` type provides this.

### `get_metric_tags`
- [ ] **CLI**: Not implemented

**MCP**: Calls `PublicApi::Metrics#tag_combinations` → `GET /api/v2/metrics/type_and_tags/:site_id/:metric_name`

**Returns**: Metric type (GAUGE/COUNTER/MEASUREMENT) and available tag combinations.

**TODO**:
- [ ] Add `metrics tags` subcommand
- [ ] REST API call needed

### `get_metrics_list`
- [ ] **CLI**: Not implemented

**MCP**: Calls `PublicApi::Metrics#list` → `POST /api/v2/metrics/list`

**Parameters**: app, metric_name, metric_type (COUNT/COUNTER/GAUGE/MEAN/P90/P95), tags, start, end.

**Returns**: Aggregated metric values for the time range.

**TODO**:
- [ ] Add `metrics list` subcommand
- [ ] REST API POST call needed
- [ ] Check if GraphQL `metrics { list(...) }` on `App` type provides equivalent functionality

### `get_metrics_timeseries`
- [ ] **CLI**: Not implemented

**MCP**: Calls `PublicApi::Metrics#timeseries` → `POST /api/v2/metrics/timeseries`

**Parameters**: Same as `get_metrics_list` but returns time-series data points.

**TODO**:
- [ ] Add `metrics timeseries` subcommand
- [ ] REST API POST call needed
- [ ] Check if GraphQL `metrics { timeseries(...) }` on `App` type provides equivalent functionality

---

## Dashboards — Write

### `manage_dashboard`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `title` | string | yes | |
| `description` | string | no | |
| `id` | string | no | If provided, updates existing; otherwise creates new |

**TODO**:
- [ ] Add `dashboards create` / `dashboards update` subcommands
- [ ] Discover GraphQL mutations for dashboard CRUD

### `create_dashboard_visual`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `dashboard_id` | string | yes | |
| `title` | string | yes | |
| `description` | string | no | |
| `metrics` | array | yes | Each: {name, fields: [{field}], tags: [{key, value}]} |
| `display` | string | no | "line", "area", "area_relative" |
| `line_label` | string | no | Supports %name%, %field%, %tag% |
| `format` | string | no | "duration", "size", "percentage", etc. |
| `draw_null_as_zero` | boolean | no | |
| `min_y_axis` | number | no | |
| `tags` | array | no | Global tags |

**TODO**:
- [ ] Add `dashboards add-visual` subcommand
- [ ] Discover GraphQL mutations

### `update_dashboard_visual`
- [ ] **CLI**: Not implemented

**TODO**:
- [ ] Add `dashboards update-visual` subcommand
- [ ] Discover GraphQL mutations

---

## Logs

### `logs tail`
- [x] **CLI**: `logs tail --app <name> --environment <env>`

Real-time log tailing via 1-second polling of the GraphQL `logs.lines` field. Supports all filters:
- `--query` — free-text search query
- `--severities` — comma-separated severity levels (e.g. "ERROR,CRITICAL")
- `--source-ids` — comma-separated source IDs
- `--view` — log view name or ID (applies the view's saved filters as defaults; CLI flags override)

Deduplicates log lines by ID across polls. Starts with 60 seconds of historical context.

### `logs search`
- [x] **CLI**: `logs search --app <name> --environment <env>`

One-shot log query designed for both human and LLM consumption. Supports:
- All filters from `logs tail` (`--query`, `--severities`, `--source-ids`, `--view`)
- `--start` / `--end` — ISO 8601 time range
- `--limit` — max lines (up to 100, default 100)
- `--order` — ASC or DESC (default DESC)
- `--json` — output as JSON for programmatic/LLM consumption

### `logs views`
- [x] **CLI**: `logs views --app <name> --environment <env>`

Lists saved log views (filter presets) for an app. Shows ID, name, query, and severities.

### `logs sources`
- [x] **CLI**: `logs sources --app <name> --environment <env>`

Lists log sources for an app. Shows ID, name, type, and format.

**Notes**: The GraphQL `logs.lines` field is capped at 100 results per query. The `--view` flag resolves a log view by name (case-insensitive) or ID and applies its saved query, source IDs, and severities as defaults. CLI flags always take precedence over view defaults.

---

## Not Applicable for CLI

### `get_more_tools`
This is an internal MCP tool that logs requests for missing capabilities. Not relevant for the CLI.

---

## Suggested Implementation Order

### Phase 1: Incident Management (Core Workflow)
1. Enhance `incidents list` with missing filters (namespaces, start/end, multiple states)
2. Add `incidents update` (state, severity, assignees)
3. Add `incidents add-note`
4. Add `apps resources` (needed for user IDs in assignment flow)

### Phase 2: Anomaly Detection
5. Add anomaly incident listing
6. Add `triggers list`

### Phase 3: Metrics
7. Add `metrics names`
8. Add `metrics tags`
9. Add `metrics timeseries`
10. Add `metrics list` (aggregated values)
11. Add `metrics discover`

### Phase 4: Dashboards
12. Add `dashboards create` / `dashboards update`
13. Add `dashboards add-visual` / `dashboards update-visual`

### Blockers & Research Needed
- **GraphQL mutations**: Need introspection to discover if mutations exist for:
  - Incident state/severity/assignee updates
  - Logbook note creation
  - Dashboard CRUD
  - Visual CRUD
- **REST API**: Metrics endpoints use a REST API (`/api/v2/metrics/...`), not GraphQL. The CLI may need a second HTTP client for these.
- **Anomaly states**: Different from exception states — need to verify GraphQL enum values.
- **Metric categories**: The MCP uses server-side YAML config for category metadata. The CLI won't have access to this; may need to hardcode or skip category-level discovery.
