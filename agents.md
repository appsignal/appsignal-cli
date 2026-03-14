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
    mod.rs             Re-exports command modules
    auth.rs            auth login / logout / status
    apps.rs            apps list / info / find / set-org / show-org
    incidents.rs       incidents list / show
```

- **CLI framework**: clap v4 with derive macros
- **HTTP client**: reqwest v0.12 with JSON + native-tls
- **Async runtime**: tokio
- **Error handling**: anyhow with contextual messages
- **Config format**: TOML via the `toml` crate
- **Config location**: `~/.config/appsignal/config.toml` (via `dirs` crate)

## AppSignal API

All API interaction goes through the **GraphQL endpoint**:

```
POST https://appsignal.com/graphql?token=<personal-api-token>
```

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

### Enums

- `IncidentStateEnum`: `OPEN`, `CLOSED`, `WIP`
- `IncidentOrderEnum`: `ID` (creation order), `LAST` (most recent activity)

### Documented GraphQL queries from the AppSignal docs

Root query fields:

- `app(id: String!)` — single app by ID
- `organization(slug: String!)` — organization by slug
- `viewer` — authenticated user info including `organizations`

Key fields on `App`:
- `id`, `name`, `environment`
- `incidents(namespaces, marker, state, actionName, limit, offset, order)` — all incident types (union)
- `incident(incidentNumber: Int!)` — single incident by number (union)
- `exceptionIncidents(namespaces, marker, query, state, limit, offset, order, actionName)`
- `performanceIncidents(namespaces, marker, query, state, limit, offset, order, actionName)`
- `anomalyIncidents(state, limit, offset, order)`
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

The org slug is used as a default for all commands that need an organization.
It can always be overridden with `--org <slug>`.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login [--token TOKEN]` | Store API token (prompts if omitted), validates via `{ __typename }` |
| `appsignal-cli auth logout` | Delete stored credentials |
| `appsignal-cli auth status` | Show auth status (masked token) |
| `appsignal-cli apps list --org <slug>` | List apps in an organization (saves org as default) |
| `appsignal-cli apps info --app-id <id>` | Show details for a single app by ID |
| `appsignal-cli apps find --name <name> [--environment <env>] [--org <slug>]` | Find app by name (case-insensitive) |
| `appsignal-cli apps set-org --org <slug>` | Set the default organization slug |
| `appsignal-cli apps show-org` | Show the current default organization |
| `appsignal-cli incidents list --app <name> [--environment <env>] [--limit N] [--state STATE] [--order ORDER]` | List incidents for an app by name |
| `appsignal-cli incidents list --app-id <id> [--limit N] [--state STATE] [--order ORDER]` | List incidents for an app by ID |
| `appsignal-cli incidents show --number <N> --app <name> [--environment <env>]` | Show incident details by number |
| `appsignal-cli incidents show --number <N> --app-id <id>` | Show incident details by number and app ID |

### App resolution

All incident commands accept either:
- `--app-id <id>` — direct app ID (takes priority)
- `--app <name> --environment <env>` — look up app by name and environment within the saved org

The `--environment` flag is only needed when multiple apps share the same name
(e.g. "Weekmenu" in both "development" and "production").

### LLM workflow example

An LLM answering "What's the latest incident for Weekmenu Production?" would run:

```bash
# Step 1: List apps (one-time, saves org for future calls)
appsignal-cli apps list --org jeroen-test-org

# Step 2: Get the latest incident
appsignal-cli incidents list --app "Weekmenu" --environment "production" --limit 1 --order LAST

# Step 3: Get full details if needed
appsignal-cli incidents show --number 1 --app "Weekmenu" --environment "production"
```

After the org is saved, step 1 is no longer needed. So the LLM can just run:

```bash
appsignal-cli incidents list --app "Weekmenu" --environment "production" --limit 1 --order LAST
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
    }
  }
}
```

The GraphQL endpoint returns HTTP 400 with error details for invalid fields,
which makes it easy to iterate, but avoid guessing field names in production
queries.
