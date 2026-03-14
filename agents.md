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
    apps.rs            apps list / apps info
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
- Listing apps requires going through `organization(slug) { apps { ... } }`,
  which means the user must provide their **organization slug** (from the
  AppSignal URL: `appsignal.com/<org-slug>`).
- The `App` type does **not** have a `language` field. Known fields on `App`:
  `id`, `name`, `environment`. Use introspection to discover additional fields
  before adding them to queries.
- Token validation uses `{ __typename }` which is a safe introspection query
  that succeeds for any valid token.
- GraphQL errors are returned with HTTP 400, not in the `errors` array of a
  200 response. The client handles both cases.

### Documented GraphQL queries from the AppSignal docs

These are the root query fields known to exist:

- `app(id: String!)` — single app by ID
- `organization(slug: String!)` — organization by slug

Known fields on `App` (via `app` or `organization.apps`):
- `id`, `name`, `environment`
- `deployMarkers(limit, offset, start, end)`
- `exceptionIncidents(namespaces, marker, limit, state, offset, order)`
- `incident(incidentNumber: Int!)`
- `metrics { list(...) }`, `metrics { timeseries(...) }`
- `uptimeMonitors`

Known fields on `Organization`:
- `apps`
- `search(query, namespace, sampleType)`

See https://docs.appsignal.com/api/graphql/examples.html for full examples.

There is also a **REST API** at `https://appsignal.com/api/[app_id]/...` for:
- `/graphs.json` — graph data (mean, count, ex_count, ex_rate, pct)
- `/markers.json` — deploy markers
- `/samples.json` — transaction samples
- `/event_names.json` — event names
- `/sourcemaps` — sourcemap uploads

The REST API uses the same `?token=` query parameter for auth.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login [--token TOKEN]` | Store API token (prompts if omitted), validates via `{ __typename }` |
| `appsignal-cli auth logout` | Delete stored credentials |
| `appsignal-cli auth status` | Show auth status (masked token) |
| `appsignal-cli apps list --org <slug>` | List apps in an organization |
| `appsignal-cli apps info --app-id <id>` | Show details for a single app |

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
