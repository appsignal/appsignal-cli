# appsignal-cli

Rust CLI for interacting with AppSignal. Binary name is `appsignal-cli` (not
`appsignal`, to avoid conflicts with the AppSignal Ruby gem).

## Architecture

```
src/
  main.rs              CLI entrypoint, clap derive command/subcommand definitions
  config.rs            Config load/save/delete (~/.config/appsignal/config.toml)
  api.rs               AppSignalClient — GraphQL client for the AppSignal API
  commands/
    mod.rs             Shared helpers (resolve_org) + re-exports
    auth.rs            auth login / logout / status
    apps.rs            apps list / info / find / set-org / show-org / orgs
    incidents.rs       incidents list / list-exceptions / list-anomalies / show
```

- **CLI framework**: clap v4 with derive macros
- **HTTP client**: reqwest v0.12 with JSON + native-tls
- **Async runtime**: tokio
- **Error handling**: anyhow with contextual messages
- **Config format**: TOML via the `toml` crate
- **Config location**: `~/.config/appsignal/config.toml` (via `dirs` crate)
- **Testing**: wiremock for HTTP mocking, tempfile for config tests

## AppSignal API

All API interaction goes through the **GraphQL endpoint**:

```
POST https://appsignal.com/graphql?token=<personal-api-token>
```

The endpoint is configurable via the `endpoint` field in config.toml
(defaults to `https://appsignal.com/graphql`).

Authentication uses a **personal API token** passed as the `token` query
parameter. Tokens can be found at https://appsignal.com/users/edit.

### Key schema facts (learned the hard way)

- There is **no** `apps` root query. The root query type only has `app(id: ...)`
  for a single app and `organization(slug: ...)` for org-level access.
- There **is** a `viewer` root query that returns the authenticated user's
  `organizations` list (each with `slug` and `name`). This is used by
  `list_organizations()` to auto-discover available orgs.
- Listing apps requires going through `organization(slug) { apps { ... } }`,
  which means the user must provide their **organization slug** (from the
  AppSignal URL: `appsignal.com/<org-slug>`).
- The `App` type has these known fields: `id`, `name`, `environment`, `status`,
  `createdAt`, `incidents`, `exceptionIncidents`, `performanceIncidents`,
  `anomalyIncidents`, `logIncidents`, `deployMarkers`, `metrics`, and more.
- `name` and `environment` are `NON_NULL String` on `App`.
- Token validation uses `{ __typename }` which is a safe introspection query
  that succeeds for any valid token.
- GraphQL errors are returned with HTTP 400, not in the `errors` array of a
  200 response. The client handles both cases.
- The GraphQL API does **not** expose `start`/`end` time range filters for
  incidents. The MCP server achieves this via direct MongoDB access.
- The GraphQL `state` filter only accepts a **single** `IncidentStateEnum`
  value, not a list (unlike the MCP server which supports comma-separated states).

### Incident types

`Incident` is a **GraphQL union** of four types:

- `ExceptionIncident` — error/exception incidents
- `PerformanceIncident` — slow action incidents
- `AnomalyIncident` — anomaly detection incidents
- `LogIncident` — log-based incidents

Common fields across all incident types:
- `id`, `number` (Int!), `state` (IncidentStateEnum), `severity` (IncidentSeverityEnum)
- `description`, `count` (Int!), `createdAt`, `lastOccurredAt`, `updatedAt`

Extra fields on `ExceptionIncident`:
- `exceptionName`, `exceptionMessage`, `actionNames`, `namespace`, `firstBacktraceLine`

Extra fields on `PerformanceIncident`:
- `actionNames`, `namespace`, `mean` (Float!), `totalDuration` (Float!)

Extra fields on `AnomalyIncident`:
- `alertState` (AlertStateEnum), `trigger` (Trigger object with `id`, `name`, `metricName`, `kind`)
- `tags` (list of `KeyStringValue` with `key` and `value`)

### Enums

- `IncidentStateEnum`: `OPEN`, `CLOSED`, `WIP`
- `IncidentOrderEnum`: `ID` (creation order), `LAST` (most recent activity)
- `AlertStateEnum`: `OPEN`, `CLOSED`, `WARMUP`, `COOLDOWN`, `UNTRACKED`, `ARCHIVED`

### Documented GraphQL queries from the AppSignal docs

Root query fields:

- `app(id: String!)` — single app by ID
- `organization(slug: String!)` — organization by slug
- `viewer` — authenticated user info including `organizations`

Key fields on `App`:
- `id`, `name`, `environment`
- `incidents(namespaces, marker, state, actionName, limit, offset, order)` — all incident types (union)
- `incident(incidentNumber: Int!)` — single incident by number (union)
- `exceptionIncidents(namespaces, marker, query, state, limit, offset, order, actionName)` — exception incidents only, with text search via `query`
- `performanceIncidents(namespaces, marker, query, state, limit, offset, order, actionName)`
- `anomalyIncidents(state, limit, offset, order)` — anomaly incidents only (fewer filters than exceptions)
- `logIncidents(state, limit, offset, order)`
- `deployMarkers(limit, offset, start, end)`
- `metrics { list(...) }`, `metrics { timeseries(...) }`
- `uptimeMonitors`

See https://docs.appsignal.com/api/graphql/examples.html for full examples.

There is also a **REST API** at `https://appsignal.com/api/[app_id]/...` for:
- `/graphs.json` — graph data (mean, count, ex_count, ex_rate, pct)
- `/markers.json` — deploy markers
- `/samples.json` — transaction samples
- `/event_names.json` — event names
- `/sourcemaps` — sourcemap uploads

The REST API uses the same `?token=` query parameter for auth.

## Config

The config file at `~/.config/appsignal/config.toml` stores:

- `token` — personal API token (set via `auth login`)
- `org` — default organization slug (auto-saved by `apps list`, or set via `apps set-org`)
- `endpoint` — (optional) custom GraphQL endpoint URL, defaults to `https://appsignal.com/graphql`

The org slug is used as a default for all commands that need an organization.
It can always be overridden with `--org <slug>`.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login [--token TOKEN]` | Store API token (prompts if omitted), validates via `{ __typename }` |
| `appsignal-cli auth logout` | Delete stored credentials |
| `appsignal-cli auth status` | Show auth status (masked token) |
| `appsignal-cli apps orgs` | List all organizations you have access to |
| `appsignal-cli apps list --org <slug>` | List apps in an organization (saves org as default) |
| `appsignal-cli apps info --app-id <id>` | Show details for a single app by ID |
| `appsignal-cli apps find --name <name> [--environment <env>] [--org <slug>]` | Find app by name (case-insensitive) |
| `appsignal-cli apps set-org --org <slug>` | Set the default organization slug |
| `appsignal-cli apps show-org` | Show the current default organization |
| `appsignal-cli incidents list [options]` | List all incident types for an app |
| `appsignal-cli incidents list-exceptions [options]` | List exception incidents (supports `--query` text search) |
| `appsignal-cli incidents list-anomalies [options]` | List anomaly detection incidents (shows trigger/alert info) |
| `appsignal-cli incidents show --number <N> [app options]` | Show full details for a specific incident |
| `appsignal-cli incidents update --number <N> [--state S] [--severity S] [--assign IDs] [--description D]` | Update incident state, severity, or assignees |
| `appsignal-cli incidents add-note --number <N> --content "..."` | Add a note to an incident (markdown supported) |

### App resolution

All incident commands accept either:
- `--app-id <id>` — direct app ID (takes priority)
- `--app <name> --environment <env>` — look up app by name and environment within the saved org

The `--environment` flag is only needed when multiple apps share the same name
(e.g. "Weekmenu" in both "development" and "production").

### Incident listing options

Common options for `incidents list`, `list-exceptions`, and `list-anomalies`:

| Flag | Description |
|---|---|
| `--app <name>` | App name (case-insensitive) |
| `--environment <env>` | Environment filter (e.g. "production") |
| `--app-id <id>` | App ID (alternative to --app) |
| `--org <slug>` | Organization (uses saved default if omitted) |
| `--limit <N>` | Max results (default: 10) |
| `--offset <N>` | Pagination offset |
| `--state <STATE>` | Filter by state: `OPEN`, `CLOSED`, or `WIP` |
| `--order <ORDER>` | Sort by: `LAST` (recent activity) or `ID` (creation) |

Additional options for `incidents list` and `list-exceptions`:

| Flag | Description |
|---|---|
| `--namespaces <ns>` | Filter by namespaces (comma-separated, e.g. "web,background") |
| `--action <name>` | Filter by action name (e.g. "UsersController#show") |

Additional option for `list-exceptions` only:

| Flag | Description |
|---|---|
| `--query <text>` | Text search for exception name or message |

### LLM workflow examples

**Finding the latest incident:**
```bash
# One-time setup: save the org slug
appsignal-cli apps list --org my-org

# Get the latest incident
appsignal-cli incidents list --app "MyApp" --environment "production" --limit 1 --order LAST

# Get full details
appsignal-cli incidents show --number 42 --app "MyApp" --environment "production"
```

**Searching for a specific error:**
```bash
appsignal-cli incidents list-exceptions --app "MyApp" --environment "production" --query "TimeoutError" --state OPEN
```

**Checking anomaly alerts:**
```bash
appsignal-cli incidents list-anomalies --app "MyApp" --environment "production" --state OPEN
```

**Filtering by namespace:**
```bash
appsignal-cli incidents list --app "MyApp" --environment "production" --namespaces "background" --state OPEN
```

**Closing an incident:**
```bash
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --state CLOSED
```

**Triaging an incident with severity and a note:**
```bash
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --severity CRITICAL
appsignal-cli incidents add-note --number 42 --app "MyApp" --environment "production" --content "Investigated: root cause is a memory leak in the connection pool."
```

## Adding new GraphQL queries

When adding new fields to queries, **always verify the field exists** on the
type first. The AppSignal GraphQL schema is not publicly documented in full.
Use introspection queries to discover available fields:

```graphql
{
  __type(name: "App") {
    fields {
      name
      type { name kind }
      args { name type { name kind } }
    }
  }
}
```

The GraphQL endpoint returns HTTP 400 with error details for invalid fields,
which makes it easy to iterate, but avoid guessing field names in production
queries.

### Known GraphQL limitations vs MCP server

The MCP server (`devenv/appsignal-server/app/mcp/tools/`) has direct MongoDB
access, giving it capabilities the GraphQL API does not expose:

- **Time range filters** (`start`/`end`) on incident queries — not available in GraphQL
- **Multiple state filters** (comma-separated) — GraphQL only accepts a single enum value
- **Trigger ID filter** on anomaly incidents — not available in GraphQL
- **Incident mutations** (update state/severity/assignees, create notes) — need to discover GraphQL mutations
- **Metrics REST API** — metric names, tags, timeseries, and aggregations use a separate REST API (`/api/v2/metrics/...`)

See `TODO.md` for the full feature parity tracking document.
