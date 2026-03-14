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
- [~] **CLI**: `incidents list`

**MCP parameters**:
| Parameter | MCP | CLI | Notes |
|---|---|---|---|
| `app_name` | required | `--app` | Case-insensitive in CLI |
| `app_environment` | required | `--environment` | Case-insensitive in CLI |
| `namespaces` | comma-separated string | Not implemented | Add `--namespaces` filter |
| `start` | ISO 8601 | Not implemented | Add `--start` filter |
| `end` | ISO 8601 | Not implemented | Add `--end` filter |
| `states` | comma-separated string | `--state` (single) | Extend to support multiple states |
| `page` | integer (1-based) | `--offset` | Different pagination model. Add `--page` as alias? |

**Missing from CLI**:
- [ ] `--namespaces` filter (comma-separated)
- [ ] `--start` / `--end` time range filters
- [ ] Support multiple states (comma-separated)
- [ ] Page-based pagination (currently offset-based)

### `get_anomaly_incidents`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `state` | string | no | "open", "closed", "warmup", "cooldown", "archived" |
| `trigger_id` | string | no | Filter by trigger |
| `page` | integer | no | 1-based, 50 per page |

**TODO**:
- [ ] Add `incidents list-anomalies` subcommand (or `--type anomaly` flag)
- [ ] Requires discovering if `anomalyIncidents` GraphQL field supports these filters
- [ ] Anomaly states differ from exception states ("warmup", "cooldown", "archived")

### `get_incident`
- [x] **CLI**: `incidents show --number <N>`

**Notes**: CLI currently fetches all incident types (exception, performance, anomaly, log) via the `incident(incidentNumber)` GraphQL union query. The MCP server only supports exception and anomaly incidents in `get_incident`.

---

## Incidents — Write

### `update_incidents`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `incidents` | array of numbers | yes | Incident numbers to update |
| `state` | string | no | "open", "closed", "wip" |
| `severity` | string | no | "critical", "high", "low", "none", "informational", "untriaged" |
| `assign_users` | array of strings | no | User IDs |
| `unassign_users` | array of strings | no | User IDs |

**TODO**:
- [ ] Add `incidents update` subcommand
- [ ] Discover GraphQL mutations for incident state/severity/assignee changes
- [ ] Supports bulk updates (multiple incident numbers)

### `create_incident_note`
- [ ] **CLI**: Not implemented

**MCP parameters**:
| Parameter | Type | Required | Notes |
|---|---|---|---|
| `number` | number | yes | Incident number |
| `app_name` | string | yes | |
| `app_environment` | string | yes | |
| `content` | string | yes | Markdown supported |

**TODO**:
- [ ] Add `incidents add-note` subcommand
- [ ] Discover GraphQL mutation for creating logbook notes

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
