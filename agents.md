# appsignal-cli

Rust CLI for interacting with AppSignal. Binary name is `appsignal-cli` (not
`appsignal`, to avoid conflicts with the AppSignal Ruby gem).

## Architecture

```
src/
  main.rs              CLI entrypoint, clap derive command/subcommand definitions
  config.rs            Config load/save/delete (~/.config/appsignal/config.toml)
  api.rs               AppSignalClient — GraphQL client for the AppSignal API
  oauth.rs             OAuth PKCE flow (code verifier, challenge, token exchange, refresh)
  commands/
    mod.rs             Shared helpers (resolve_org, authenticated_client) + re-exports
    auth.rs            auth login / logout / status (supports both token and OAuth)
    apps.rs            apps list / info / find / set-org / show-org / orgs
    incidents.rs       incidents list / list-exceptions / list-performance / list-anomalies / show
    logs.rs            logs tail / search / views / sources
    skill.rs           skill install (writes bundled AppSignal skills for OpenCode, Codex, or Claude)
```

- **CLI framework**: clap v4 with derive macros
- **HTTP client**: reqwest v0.12 with JSON + rustls-tls
- **Async runtime**: tokio
- **Error handling**: anyhow with contextual messages
- **Config format**: TOML via the `toml` crate
- **Config location**: `~/.config/appsignal/config.toml` (via `dirs` crate)
- **Time handling**: chrono for UTC timestamps in log tailing/pagination
- **Testing**: wiremock for HTTP mocking, tempfile for config tests

## Rust Workflow

- Before committing Rust changes, run `cargo fmt` and `cargo clippy -- -D warnings`.

## Git Workflow

- Prefer descriptive branch names. Avoid cliché names like `fix-bug` or `wip`, keep
  names longer than 3 characters, avoid leading/trailing punctuation, and do not
  use a ticket number as the whole branch name.
- Prefer rebasing feature branches instead of merging base branches back into
  them. Avoid local merge commits from `git pull`; use `git pull --rebase` (or a
  Git config like `pull.rebase=true` or `pull.ff=only`) instead.
- Do not leave `fixup!`, `squash!`, or amend-style commits in a branch that is
  ready for review. Autosquash them before pushing or merging.
- Write commit subjects in imperative mood, with a capitalized first letter,
  without trailing punctuation, and keep them under 50 characters.
- Do not use conventional-commit prefixes like `fix:` or `feat:` in the subject.
  Do not put ticket numbers, `[skip ci]`, or similar tags in the subject either;
  move that metadata into the commit body.
- Avoid vague subjects like `Fix bug`, `Update README`, or `WIP`. The subject
  should describe the actual change.
- Always include a commit body separated from the subject by a blank line.
  Explain why the change was needed, what was changed, and any important
  constraints or rejected alternatives.
- Wrap commit body lines at 72 characters when practical. URLs may stay on one
  line.
- Put ticket references like `Closes #123` or `Related #123` in the body, not in
  the subject.
- Keep trailer lines like `Co-authored-by:` and `Signed-off-by:` at the end of
  the commit body.
- In this repo, when a code change does not need a user-facing changeset,
  include `[skip changeset]` in the commit body rather than the subject.

## AppSignal API

The CLI currently uses a mix of **GraphQL** and **REST v2** endpoints:

```text
GraphQL: POST https://appsignal.com/graphql
REST v2: https://appsignal.com/api/v2/...
```

The configured `endpoint` value is a base URL.
It defaults to `https://appsignal.com/`, and the client derives `/graphql` or
`/api/v2/...` paths from that base.

### Authentication

The CLI supports two authentication methods:

1. **Personal API token** — passed as a `?token=` query parameter.
   Tokens can be found at https://appsignal.com/users/edit.
2. **OAuth (PKCE)** — access token sent via `Authorization: Bearer` header.
   Obtained through the OAuth authorization code flow with PKCE.

OAuth credentials take precedence when both are present in the config.

The `AppSignalClient` struct stores an `AuthMethod` enum that determines how
each request is authenticated:
- GraphQL with `AuthMethod::PersonalToken(token)` → appends `?token=<token>` to the URL
- GraphQL with `AuthMethod::OAuth { access_token, .. }` → sets `Authorization: Bearer <token>`
- REST v2 with either auth method → sets `Authorization: Bearer <token>`

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

There is also a legacy **REST API** at `https://appsignal.com/api/[app_id]/...` for:
- `/graphs.json` — graph data (mean, count, ex_count, ex_rate, pct)
- `/markers.json` — deploy markers
- `/samples.json` — transaction samples
- `/event_names.json` — event names
- `/sourcemaps` — sourcemap uploads

That legacy REST API uses the same `?token=` query parameter for auth (personal
tokens) or `Authorization: Bearer` header (OAuth tokens).

## Config

The global config file at `~/.config/appsignal/config.toml` stores:

- `token` — personal API token (set via `auth login --token`)
- `org` — default organization slug (auto-saved by `apps list`, or set via `apps set-org`)
- `endpoint` — (optional) custom AppSignal base URL, defaults to `https://appsignal.com/`
- `oauth_client_id` — (optional) OAuth client ID override; defaults to the production client ID when unset
- `[oauth]` — OAuth credentials (set via `auth login --oauth`):
  - `access_token` — OAuth access token
  - `refresh_token` — OAuth refresh token (used for automatic renewal)
  - `expires_at` — UNIX timestamp when the access token expires

When OAuth credentials are present, they take precedence over the personal token.
Switching auth methods clears the other (i.e., `--oauth` clears `token`, `--token` clears `[oauth]`).

The org slug is used as a default for all commands that need an organization.
It can always be overridden with `--org <slug>`.

If the current directory (or one of its parents) contains `.appsignal.toml`, the
CLI uses that nearest local file as the only config for the project. Config
reads and writes go to the active file, so project-specific auth, endpoint, and
org settings stay isolated from the global config.

## OAuth

The CLI uses the **OAuth 2.0 Authorization Code flow with PKCE** for browser-based
authentication. Implementation is in `src/oauth.rs`.

### Flow

1. CLI generates a PKCE code verifier + S256 challenge and a random state parameter
2. Opens the browser to `<oauth-base>/oauth/authorize` with PKCE and state params
3. User authorizes in the browser; server redirects to the configured callback URI
4. The CLI receives the callback automatically on `http://127.0.0.1:9789/callback`
5. CLI validates the state, then exchanges the code for tokens via `POST <oauth-base>/oauth/token`
6. Credentials are saved to `config.toml`

### Configuration

| Setting | Value |
|---|---|
| Default client ID | `FpXP78S_vXNrjSRYQIMWQ9sREl2AXS0qD0VSWwfHST0` |
| Default redirect URI | `http://127.0.0.1:9789/callback` |
| Scopes | `app:read app:write` |
| PKCE method | S256 |
| Grant types | `authorization_code`, `refresh_token` |

The OAuth base defaults to `https://appsignal.com` and is derived from `endpoint`
when a custom endpoint is configured, for example `https://staging.lol`.

The CLI uses the production OAuth client ID by default for all builds. To use a
staging-specific OAuth client, set `oauth_client_id` in `config.toml`.

The OAuth provider is Doorkeeper, configured in `appsignal-server`. The CLI
application is registered as a **non-confidential** (public) client — no client
secret is used.

### Token refresh

Before each API call, `commands::authenticated_client()` checks if the stored
OAuth access token has expired (with a 60-second grace window). If expired and
a refresh token is available, it automatically refreshes the token and persists
the updated credentials. If refresh fails, the user is prompted to re-authenticate.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login --oauth [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Authenticate via OAuth PKCE flow using the active config for the current project or the global config |
| `appsignal-cli auth login [--token TOKEN] [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Store personal API token and optional config overrides in the active config, validates via `{ __typename }` |
| `appsignal-cli auth logout` | Delete stored credentials from the active config |
| `appsignal-cli auth status` | Show auth status, method (OAuth/token), and expiry |
| `appsignal-cli project init [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Create or update the project-local `.appsignal.toml`, which becomes the only config used in that project |
| `appsignal-cli apps orgs` | List all organizations you have access to |
| `appsignal-cli apps list --org <slug>` | List apps in an organization and save the default org to the active config |
| `appsignal-cli apps info --app-id <id>` | Show details for a single app by ID |
| `appsignal-cli apps find --name <name> [--environment <env>] [--org <slug>]` | Find app by name (case-insensitive) |
| `appsignal-cli apps set-org --org <slug>` | Set the default organization slug in the active config |
| `appsignal-cli apps show-org` | Show the current default organization |
| `appsignal-cli incidents list [options]` | List all incident types for an app |
| `appsignal-cli incidents list-exceptions [options]` | List exception incidents (supports `--query` text search) |
| `appsignal-cli incidents list-performance [options]` | List performance incidents (supports `--query` text search) |
| `appsignal-cli incidents list-anomalies [options]` | List anomaly detection incidents (shows trigger/alert info) |
| `appsignal-cli incidents show --number <N> [app options]` | Show full details for a specific incident |
| `appsignal-cli incidents update --number <N> [--state S] [--severity S] [--assign IDs] [--description D]` | Update incident state, severity, or assignees |
| `appsignal-cli incidents add-note --number <N> --content "..."` | Add a note to an incident (markdown supported) |
| `appsignal-cli logs tail [filters]` | Stream log lines in real time (1-second polling) |
| `appsignal-cli logs search [filters] [--page-all]` | One-shot log search (supports auto-pagination and global `--output json`) |
| `appsignal-cli logs views [app options]` | List saved log views (filter presets) |
| `appsignal-cli logs sources [app options]` | List log sources for an app |
| `appsignal-cli skill install [--target TARGET] [--dir PATH] [--force]` | Install the bundled AppSignal LLM skill for OpenCode, Codex, or Claude |
| `appsignal-cli skill update [--target TARGET] [--dir PATH]` | Update an installed AppSignal LLM skill to the bundled version |
| `appsignal-cli skill status [--target TARGET] [--dir PATH]` | Show whether installed AppSignal LLM skills are current, outdated, missing, or unversioned; defaults to all supported targets |

### App resolution

All incident commands accept either:
- `--app-id <id>` — direct app ID (takes priority)
- `--app <name> --environment <env>` — look up app by name and environment within the saved org

The `--environment` flag is only needed when multiple apps share the same name
(e.g. "Weekmenu" in both "development" and "production").

### Incident listing options

Common options for `incidents list`, `list-exceptions`, `list-performance`, and `list-anomalies`:

| Flag | Description |
|---|---|
| `--app <name>` | App name (case-insensitive) |
| `--environment <env>` | Environment filter (e.g. "production") |
| `--app-id <id>` | App ID (alternative to --app) |
| `--org <slug>` | Organization (uses saved default if omitted) |
| `--limit <N>` | Max results (default: 10) |
| `--offset <N>` | Pagination offset |
| `--state <STATE>` | Filter by state: `OPEN`, `CLOSED`, or `WIP` |
| `--order <ORDER>` | Sort by: `LAST` (recent activity, default) or `ID` (creation) |

Additional options for `incidents list`, `list-exceptions`, and `list-performance`:

| Flag | Description |
|---|---|
| `--namespaces <ns>` | Filter by namespaces (comma-separated, e.g. "web,background") |
| `--action <name>` | Filter by action name (e.g. "UsersController#show") |

Additional option for `list-exceptions` and `list-performance`:

| Flag | Description |
|---|---|
| `--query <text>` | Text search for name or message |

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

**Searching for slow actions:**
```bash
appsignal-cli incidents list-performance --app "MyApp" --environment "production" --query "UsersController" --state OPEN
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

## Logs

### Architecture

Log data lives in **ClickHouse** (not MongoDB). The AppSignal server has two paths
for querying logs:

1. **GraphQL** (`app.logs.lines`) — deprecated and removed server-side.
2. **REST API** (`POST /api/v2/logs/lines`) — used by the CLI for `logs search`
   and `logs tail`. Supports cursor-based pagination and SSE streaming.

The CLI still uses GraphQL for `logs views` and `logs sources` metadata.

### Key log types

**`LogLine`** — a single log entry:
- `id` (String!) — UUID
- `timestamp` (PreciseDateTime!) — ISO 8601
- `severity` (SeverityEnum!) — UNKNOWN, TRACE, DEBUG, INFO, NOTICE, WARN, ERROR, CRITICAL, ALERT, FATAL
- `hostname` (String!) — originating host
- `group` (String) — log group (e.g. "notifiers", "background")
- `message` (String!) — the log message body
- `attributes` ([KeyStringValue]) — key-value metadata
- `source` (Source) — which log source (lazy-loaded)

**`LogView`** — a saved filter preset:
- `id`, `name`, `query` (free-text search), `sourceIds`, `severities`, `columns`, `lineHeight`

**`LogSource`** — a log ingestion source:
- `id`, `name`, `type` (vector/custom/vercel), `fmt` (JSON/PLAINTEXT/LOGFMT/AUTODETECT), `key`

### Queries used

```graphql
# Fetch log views
app(id: $appId) {
  logViews { id name query sourceIds severities columns }
}

# Fetch single log view by ID
app(id: $appId) {
  logView(id: $viewId) { id name query sourceIds severities columns }
}

# Fetch log sources
app(id: $appId) {
  logs {
    sources { id name type fmt }
  }
}
```

```json
// Fetch log lines
POST /api/v2/logs/lines
{
  "site_id": "app-id",
  "from": "2025-06-01T00:00:00Z",
  "to": "2025-06-01T01:00:00Z",
  "source_ids": ["source-1"],
  "query": "message:timeout severity=[error]",
  "pagination": {
    "per_page": 100,
    "order": "desc",
    "cursor": { "time": null }
  }
}
```

### Query syntax

The `query` parameter supports a rich filter syntax parsed by the ClickHouse BulkEndpoint
Rust service. Full docs: https://docs.appsignal.com/logging/query-syntax

#### Operators

| Operator | Syntax | Meaning | Example |
|----------|--------|---------|---------|
| `=` | `field=value` | Exact match | `group=notifiers`, `severity=error` |
| `!=` | `field!=value` | Not equal | `source!=mongodb` |
| `:` | `field:value` | Contains (substring) | `message:timeout`, `hostname:prod` |
| `!:` | `field!:value` | Does not contain | `hostname!:test` |
| `>` `<` `>=` `<=` | `field>value` | Numeric comparison | `duration>100` |

#### Available fields

`severity`, `hostname`, `group`, `message`, plus any custom attribute (dot notation
for nested: `user.id=123`).

#### Free text

Bare words (no field prefix) search the `message` field: `timeout` finds messages
containing "timeout".

#### Combining

- **Spaces** = implicit AND: `severity=error hostname=web-1`
- **`OR`** keyword: `severity=error OR severity=warning`
- **Parentheses**: `severity=error AND (hostname=web-1 OR hostname=web-2)`

#### Quoting

Use quotes for values with spaces or special characters: `group:"background jobs"`,
`message:"[Email]"`.

**IMPORTANT**: Square brackets `[...]` have special meaning (list/array syntax in
the legacy query parser). If you want to search for a literal `[Email]` in messages,
you MUST use `message:"[Email]"` — NOT `[Email]` as bare text.

**Correct**: `group=notifiers message:"[Email]"`
**Wrong**: `group=notifiers [Email]` (the `[Email]` gets parsed as a list, matching
any line containing the word "email" case-insensitively, not the literal `[Email]` prefix)

#### Severities

The CLI merges the `--severities` flag into the REST query string as
`severity=[error,critical]`. Using the flag is preferred over embedding
severity filters manually in `--query`.

### Log view resolution

The `--view` flag on `tail` and `search` resolves a log view by:
1. Trying the value as a **view ID** via `get_log_view(app_id, view_id)`
2. If that fails, listing all views and matching **by name** (case-insensitive)
3. Errors if no match or multiple matches

The view's `query`, `sourceIds`, and `severities` are applied as defaults.
Explicit CLI flags (`--query`, `--severities`, `--source-ids`) always override
the view's values.

### Pagination (`--page-all`)

The REST `POST /api/v2/logs/lines` endpoint is still queried in pages of 100.
The `--page-all` flag on `logs search` iterates through those pages using the
timestamp cursor:

1. Fetch 100 lines in **ASC** order (oldest first)
2. If the page is full (100 results), use the **last line's timestamp** as the
   next page cursor
3. **Deduplicate by log line ID** using a `HashSet` — multiple lines can share
   the exact same timestamp, so the boundary between pages may include duplicates
4. Stop when a page returns fewer than 100 results

This is implemented in `fetch_all_pages()` in `commands/logs.rs`.

Progress is printed to stderr: `Page N: fetched X lines (Y total unique)`.

### Tailing (`logs tail`)

Live log tailing currently uses the REST log query endpoint with **polling**
(not SSE yet):

1. Start with a 60-second lookback window
2. Every **1 second**, query for lines between `start` and `now` in ASC order
3. Deduplicate by ID using a `HashSet`
4. Move the window forward: next `start` = `now - 120 seconds` (2-minute lookback for safety)
5. Prune the seen-ID set when it exceeds 1000 entries (re-seed from current batch)

This matches the frontend's live tail behavior in `useLogTail.js`.

### CLI commands

| Command | Description |
|---|---|
| `logs tail [filters]` | Real-time log streaming (1s poll interval) |
| `logs search [filters] [--page-all]` | One-shot log query |
| `logs views` | List saved log views for an app |
| `logs sources` | List log sources for an app |

Filter flags (shared by `tail` and `search`):
- `--query <text>` — free-text search with field filter syntax
- `--severities <list>` — comma-separated (e.g. `ERROR,CRITICAL`)
- `--source-ids <list>` — comma-separated source IDs
- `--view <name-or-id>` — apply a saved log view's filters

Additional flags for `search`:
- `--start <ISO8601>` — start time
- `--end <ISO8601>` — end time
- `--limit <N>` — max results (default 100, max 100)
- `--order <ASC|DESC>` — sort order (default DESC)
- `--output <human|json>` — global result format for programmatic/LLM consumption
- `--page-all` — auto-paginate to fetch all results (ignores --limit/--order)

### LLM workflow examples

**Search for recent errors:**
```bash
appsignal-cli --output json logs search --app "MyApp" --environment "production" \
  --severities ERROR,CRITICAL
```

**Count emails sent in a time window:**
```bash
appsignal-cli --output json logs search --app "appsignal" --environment "production" \
  --query "group:notifiers [Email]" \
  --start "2025-03-16T06:53:00Z" --page-all
```

**Tail logs with a saved view:**
```bash
appsignal-cli logs tail --app "MyApp" --environment "production" --view "Error logs"
```

**Get log sources to use as filter:**
```bash
appsignal-cli logs sources --app "MyApp" --environment "production"
# Then use a source ID:
appsignal-cli --output json logs search --app "MyApp" --environment "production" \
  --source-ids "636873bc14ad665402d297e2"
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
- **Log views/sources metadata** — still GraphQL-backed; there is no documented v2 REST
  replacement for `logs views` or `logs sources` yet.

See `TODO.md` for the full feature parity tracking document.
