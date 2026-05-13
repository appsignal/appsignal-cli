# appsignal-cli

A command-line interface for [AppSignal](https://appsignal.com), built in Rust. Designed to be used by both humans and LLMs to query AppSignal data from the terminal.

## Installation

The easiest way to get `appsignal-cli` in your machine is to run our installation one-liner:

```sh
curl -sSL https://github.com/appsignal/appsignal-cli/releases/latest/download/install.sh | sh
```

You'll need to run it with super-user privileges -- if you're not running this as root, prefix it with `sudo`.

`appsignal-cli` is only supported for Linux and macOS, in the x86_64 (Intel) and arm64 (Apple Silicon) architectures. Linux distributions based on musl, such as Alpine, are also supported.

Not a fan of `curl | sh` one-liners? Download the binary for your operating system and architecture [from our latest release](https://github.com/appsignal/appsignal-cli/releases/latest/).

## Authentication

There are two ways to authenticate:

### OAuth (recommended)

```sh
appsignal-cli auth login --oauth
```

This opens your browser to authorize the CLI with your AppSignal account. After
authorizing, the CLI waits for the browser callback on `http://127.0.0.1:9789/callback`
by default, so you usually do not need to copy anything back into the terminal.
OAuth tokens are automatically refreshed when they expire.

### Personal API token

Get your personal API token from https://appsignal.com/users/edit, then:

```sh
appsignal-cli auth login --token <your-token>
```

Credentials are stored in `~/.config/appsignal/config.toml`.

## Quick start

```sh
# List your organizations
appsignal-cli apps orgs

# List apps in an organization (saves the org as default)
appsignal-cli apps list --org <org-slug>

# Find an app by name
appsignal-cli apps find --name "MyApp" --environment "production"

# List recent incidents (all types)
appsignal-cli incidents list --app "MyApp" --environment "production" --limit 5

# List only exception incidents
appsignal-cli incidents list-exceptions --app "MyApp" --environment "production" --state OPEN

# Search exceptions by name or message
appsignal-cli incidents list-exceptions --app "MyApp" --environment "production" --query "TimeoutError"

# List performance incidents
appsignal-cli incidents list-performance --app "MyApp" --environment "production"

# List anomaly detection alerts
appsignal-cli incidents list-anomalies --app "MyApp" --environment "production"

# Show details for a specific incident
appsignal-cli incidents show --number 42 --app "MyApp" --environment "production"

# Close an incident
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --state CLOSED

# Add a note to an incident
appsignal-cli incidents add-note --number 42 --app "MyApp" --environment "production" --content "Root cause identified."

# Tail logs in real time
appsignal-cli logs tail --app "MyApp" --environment "production"

# Search logs with JSON output (for LLMs)
appsignal-cli logs search --app "MyApp" --environment "production" --query "timeout" --json

# Fetch all logs in a time range (auto-paginate)
appsignal-cli logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --query "group:notifiers" --page-all --json
```

## Commands

### `auth`

| Command | Description |
|---|---|
| `auth login --oauth` | Authenticate via OAuth (opens browser and waits for local callback) |
| `auth login [--token TOKEN]` | Authenticate with a personal API token (prompts if omitted) |
| `auth logout` | Remove stored credentials |
| `auth status` | Show authentication status and method |

### `apps`

| Command | Description |
|---|---|
| `apps orgs` | List all organizations you have access to |
| `apps list --org <slug>` | List apps in an organization (saves org as default) |
| `apps info --app-id <id>` | Show details for a specific app |
| `apps find --name <name> [--environment <env>]` | Find an app by name |
| `apps set-org --org <slug>` | Set the default organization |
| `apps show-org` | Show the current default organization |

### `incidents`

| Command | Description |
|---|---|
| `incidents list` | List all incident types for an app |
| `incidents list-exceptions` | List exception incidents (supports text search) |
| `incidents list-performance` | List performance incidents (supports text search) |
| `incidents list-anomalies` | List anomaly detection incidents |
| `incidents show --number <N>` | Show details for a specific incident |
| `incidents update --number <N>` | Update incident state, severity, or assignees |
| `incidents add-note --number <N> --content "..."` | Add a note to an incident |

### `logs`

| Command | Description |
|---|---|
| `logs tail` | Stream log lines in real time (polls every second) |
| `logs search` | Search log lines (one-shot query, supports `--json` for LLM use) |
| `logs views` | List saved log views (filter presets) |
| `logs sources` | List log sources for an app |

All log and incident commands accept either `--app-id <id>` or `--app <name> [--environment <env>]` to identify the application. The `--environment` flag is needed when multiple apps share the same name.

#### Incident common options

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

#### Additional options for `list`, `list-exceptions`, and `list-performance`

| Flag | Description |
|---|---|
| `--namespaces <ns>` | Filter by namespaces (comma-separated, e.g. "web,background") |
| `--action <name>` | Filter by action name (e.g. "UsersController#show") |

#### Additional option for `list-exceptions` and `list-performance`

| Flag | Description |
|---|---|
| `--query <text>` | Search by name or message |

#### `incidents update` options

| Flag | Description |
|---|---|
| `--state <STATE>` | New state: `OPEN`, `CLOSED`, or `WIP` |
| `--severity <SEV>` | New severity: `UNTRIAGED`, `CRITICAL`, `HIGH`, `LOW`, `NONE`, or `INFORMATIONAL` |
| `--assign <IDs>` | Comma-separated user IDs to assign |
| `--description <text>` | New description |

#### Log filtering options

All log commands (`tail`, `search`) support these filters:

| Flag | Description |
|---|---|
| `--query <text>` | Log query filter ([syntax docs](https://docs.appsignal.com/logging/query-syntax)) |
| `--severities <list>` | Comma-separated severities (e.g. `ERROR,CRITICAL`) |
| `--source-ids <list>` | Comma-separated log source IDs |
| `--view <name-or-id>` | Apply a saved log view's filters as defaults |

#### Additional options for `logs search`

| Flag | Description |
|---|---|
| `--start <ISO8601>` | Start time (e.g. `2025-01-01T00:00:00Z`) |
| `--end <ISO8601>` | End time |
| `--limit <N>` | Max results per page (default: 100, max: 100) |
| `--order <ORDER>` | `ASC` (oldest first) or `DESC` (newest first, default) |
| `--json` | Output as JSON (for LLM/programmatic consumption) |
| `--page-all` | Auto-paginate to fetch all results in the time range |

The `--view` flag resolves a log view by name (case-insensitive) or ID. CLI flags always override the view's saved defaults.

The `--page-all` flag works by slicing the time window: it fetches 100 lines at a time in ASC order, using the last line's timestamp as the start of the next request, deduplicating by log line ID at boundaries.

#### Query syntax

The `--query` flag uses AppSignal's [log query syntax](https://docs.appsignal.com/logging/query-syntax). Key patterns:

- `severity=error` — exact field match
- `message:timeout` — message contains "timeout"
- `group=notifiers` — exact group match
- `hostname:prod` — hostname contains "prod"
- `message:"[Email]"` — use quotes for special characters like `[` `]`
- Space-separated terms are combined with AND; use `OR` for alternatives

**Note:** Square brackets `[...]` have special meaning in the query parser. To search for literal brackets (e.g. `[Email]`), use `message:"[Email]"` — not `[Email]` as bare text.

#### Log examples

```sh
# Tail logs in real time
appsignal-cli logs tail --app "MyApp" --environment "production"

# Tail only error logs
appsignal-cli logs tail --app "MyApp" --environment "production" --severities ERROR,CRITICAL

# Tail using a saved log view
appsignal-cli logs tail --app "MyApp" --environment "production" --view "Error logs"

# Search recent logs
appsignal-cli logs search --app "MyApp" --environment "production" --query "timeout" --severities ERROR

# Search with time range and literal bracket matching
appsignal-cli logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --end "2025-03-16T07:00:00Z" \
  --query 'group=notifiers message:"[Email]"'

# Fetch ALL matching logs (auto-paginate beyond the 100-line API limit)
appsignal-cli logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --query 'group=notifiers message:"[Email]"' --page-all --json

# Get JSON output for LLM consumption
appsignal-cli logs search --app "MyApp" --environment "production" --query "error" --json

# List available log views
appsignal-cli logs views --app "MyApp" --environment "production"

# List log sources
appsignal-cli logs sources --app "MyApp" --environment "production"
```

## Configuration

Config is stored at `~/.config/appsignal/config.toml`:

```toml
# When using a personal API token:
token = "your-api-token"
org = "your-org-slug"

# Optional: point the CLI at a non-production AppSignal server.
# This must be the base URL, without `/graphql`.
endpoint = "https://staging.lol"

# Optional: override the default production OAuth client ID
oauth_client_id = "your-staging-client-id"

# When using OAuth (set automatically by `auth login --oauth`):
[oauth]
access_token = "..."
refresh_token = "..."
expires_at = 1742324400
```

OAuth credentials take precedence over personal tokens when both are present.
Expired OAuth tokens are automatically refreshed before API calls.
When `oauth_client_id` is unset, the CLI uses the production OAuth client ID.
When `endpoint` is set to a base URL like `https://staging.lol`, the CLI uses
`/graphql` for API calls and the base URL itself for OAuth. Values like
`https://staging.lol/graphql` are not supported.
OAuth always uses the built-in local callback at `http://127.0.0.1:9789/callback`.

The `org` value is saved automatically when you run `apps list --org <slug>` or `apps set-org --org <slug>`, so subsequent commands don't need `--org`.

## Development

```sh
# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check
```

CI runs all three checks on every push and pull request via GitHub Actions.

## License

Copyright (c) AppSignal. All rights reserved.
