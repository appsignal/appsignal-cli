# REST Migration TODO

Track which CLI commands should move from GraphQL to the AppSignal v2 REST API.

## Move To REST

### 1. Auth validation

- Command: `auth login`
- Current code:
  - `src/commands/auth.rs`
  - `src/api.rs` (`validate_token`)
- Current GraphQL usage:
  - GraphQL introspection query: `{ __typename }`
- REST replacement:
  - `GET /api/v2/auth`
- Why:
  - Dedicated auth check exists in REST
  - Better fit than probing GraphQL

### 2. Log search

- Command: `logs search`
- Current code:
  - `src/commands/logs.rs`
  - `src/api.rs` (`list_log_lines`)
- Current GraphQL usage:
  - `app.logs.lines`
- REST replacement:
  - `POST /api/v2/logs/lines`
- Why:
  - REST is the documented log query API
  - GraphQL is capped at 100 results
  - CLI currently works around this with `--page-all` time slicing

### 3. Log tailing

- Command: `logs tail`
- Current code:
  - `src/commands/logs.rs`
  - `src/api.rs` (`list_log_lines`)
- Current GraphQL usage:
  - `app.logs.lines`
- REST replacement:
  - `POST /api/v2/logs/lines`
- Why:
  - REST supports streaming/SSE
  - Current implementation uses polling and deduplication because GraphQL is not ideal for tailing

## Keep On GraphQL For Now

These commands still depend on data that is not exposed by the documented v2 REST API.

### Logs metadata

- `logs views`
- `logs sources`

Reason: no matching REST endpoints were found for log views or log sources.

### App and org discovery

- `apps list`
- `apps info`
- `apps find`
- `apps orgs`
- `apps resources all`
- `apps resources users`
- `apps resources notifiers`
- `apps resources namespaces`
- `apps resources dashboards`
- `apps resources deploy-markers`

Reason: no matching REST endpoints were found for organizations, apps, users, notifiers, namespaces, dashboards, or deploy markers listing.

### Incident commands

- `incidents list`
- `incidents list-exceptions`
- `incidents list-performance`
- `incidents list-anomalies`
- `incidents show`
- `incidents update`
- `incidents add-note`

Reason: no matching REST incident endpoints were found in the documented v2 API.

## Near-Misses / Future Work

### Performance data is available in REST, but not as incidents

Relevant REST endpoints:

- `POST /api/v2/tracing/actions`
- `POST /api/v2/tracing/traces/performance`
- `POST /api/v2/tracing/trace/performance`
- `POST /api/v2/tracing/traces/errors`
- `POST /api/v2/tracing/trace/error`

These could support new CLI commands later, but they do not directly replace the current `incidents list-performance` behavior.

### Deploy stats exist, but not deploy marker listing

Relevant REST endpoint:

- `POST /api/v2/deploys/stats`

This is not a direct replacement for `apps resources deploy-markers`.

## Refactor Notes

- `src/api.rs` currently assumes a GraphQL-first client and normalizes endpoints to `/graphql`.
- Before migrating logs and auth cleanly, split client concerns into:
  - base URL handling
  - GraphQL request helper
  - REST request helper

## First Implementation Order

1. [x] Add REST auth validation via `GET /api/v2/auth`
2. Add a REST log query client for `POST /api/v2/logs/lines`
3. Migrate `logs search`
4. Migrate `logs tail`
5. Remove or simplify GraphQL-specific log pagination workarounds if REST pagination/streaming fully replaces them
