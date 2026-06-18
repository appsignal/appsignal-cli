# appsignal-cli

A command-line interface for [AppSignal](https://appsignal.com), built in Rust.
It helps humans, scripts, and LLM agents inspect AppSignal data from the
terminal.

- [AppSignal.com website][appsignal]
- [Documentation][docs]
- [Support][contact]

## What can it do?

Use `appsignal-cli` to:

- Authenticate with AppSignal through OAuth
- List, find, and inspect AppSignal apps
- Search and tail application logs
- List, inspect, update, and annotate incidents
- Fetch the transaction samples behind an incident (by URL, id, or timestamp)
- Discover metric keys and pull metric timeseries and historical datapoints
- Rank slow performance actions and the queries behind them
- Manage dashboards, anomaly detection triggers, log-derived metrics, and
  log-based triggers
- Render command output as JSON for scripts and LLM agents

## Installation

### Homebrew

On macOS and Linux, install `appsignal-cli` with Homebrew:

```sh
brew install appsignal/appsignal-cli/appsignal-cli
```

Homebrew automatically taps `appsignal/appsignal-cli` when using the full
formula name above. You can also tap it explicitly:

```sh
brew tap appsignal/appsignal-cli
brew install appsignal-cli
```

### Install script

You can also install `appsignal-cli` with our installation one-liner:

```sh
curl -sSL https://github.com/appsignal/appsignal-cli/releases/latest/download/install.sh | sudo sh
```

The installer needs super-user privileges because it installs the binary on your
system path. It verifies the downloaded archive against the `SHA256SUMS`
manifest published with each release before extracting it.

`appsignal-cli` is supported for Linux and macOS, in the x86_64 (Intel) and arm64
(Apple Silicon) architectures. Linux distributions based on musl, such as
Alpine, are also supported.

Not a fan of `curl | sh` one-liners? Download the binary for your operating
system and architecture [from our latest release](https://github.com/appsignal/appsignal-cli/releases/latest/).

## Authentication

Authenticate with OAuth:

```sh
appsignal-cli auth login
```

This opens your browser to authorize the CLI with your AppSignal account. After
authorizing, the CLI waits for the browser callback on `http://127.0.0.1:9789/callback`
by default, so you usually do not need to copy anything back into the terminal.
OAuth tokens are automatically refreshed when they expire.

For project-specific setup, initialize `.appsignal.toml` first:

```sh
appsignal-cli project init
appsignal-cli auth login
```

You can also set a project-specific default org during initialization:

```sh
appsignal-cli project init --org your-org-slug
```

Credentials are stored in `~/.config/appsignal/config.toml` by default. Once a
project-local `.appsignal.toml` exists, commands run in that project use it
automatically.

`project init` does not copy your stored global OAuth credentials into
the local file. Authenticate afterward if you want project-specific credentials.

### Headless / CI authentication

For non-interactive environments where the OAuth browser flow is impractical,
authenticate with a personal API token instead:

```sh
# Via environment variable (recommended for CI)
export APPSIGNAL_API_TOKEN="your-personal-api-token"
appsignal-cli apps list

# Or per-invocation
appsignal-cli --api-token "your-personal-api-token" incidents list --app-id <app-id>
```

A token supplied this way takes precedence over any stored OAuth credentials and
is sent as a `?token=` query parameter; nothing is written to disk. OAuth remains
the default when no token is provided. `appsignal-cli auth status` shows which
method is active.

## Quick start

```sh
# Authenticate with AppSignal
appsignal-cli auth login

# List apps for the current account (saves the org as default)
appsignal-cli apps list

# Initialize a project-local config
appsignal-cli project init --org <org-slug>

# Find an app by name
appsignal-cli apps find --name "MyApp" --environment "production"

# List recent incidents (all types)
appsignal-cli incidents list --app "MyApp" --environment "production" --limit 5

# Show details for a specific incident
appsignal-cli incidents show --number 42 --app "MyApp" --environment "production"

# Tail logs in real time
appsignal-cli logs tail --app "MyApp" --environment "production"

# Search logs with JSON output (for LLMs)
appsignal-cli --output json logs search --app "MyApp" --environment "production" --query "timeout"

# Install the bundled AppSignal LLM skill
appsignal-cli skill install
```

## Common workflows

### Incidents

```sh
# List only exception incidents
appsignal-cli incidents list-exceptions --app "MyApp" --environment "production" --state OPEN

# Search exceptions by name or message
appsignal-cli incidents list-exceptions --app "MyApp" --environment "production" --query "TimeoutError"

# List performance incidents
appsignal-cli incidents list-performance --app "MyApp" --environment "production"

# List anomaly detection alerts
appsignal-cli incidents list-anomalies --app "MyApp" --environment "production"

# Close an incident
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --state CLOSED

# Close multiple incidents at once
appsignal-cli incidents update --number 41,42,43 --app "MyApp" --environment "production" --state CLOSED

# Assign yourself to an incident
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --assign-me

# Add a note to an incident
appsignal-cli incidents add-note --number 42 --app "MyApp" --environment "production" --content "Root cause identified."
```

### Samples

```sh
# Fetch the latest sample for an incident (prints an analysed digest)
appsignal-cli samples show --incident 42 --app "MyApp" --environment "production"

# Fetch a sample straight from an AppSignal URL
appsignal-cli samples show "https://appsignal.com/my-org/sites/<app-id>/performance/incidents/42"

# Fetch the sample closest to a known incident time (better for retrospectives than "latest")
appsignal-cli samples show --incident 42 --app-id <app-id> --at "2026-05-19T14:30:00Z"

# Show the unprocessed sample instead of the digest
appsignal-cli samples show --incident 42 --app-id <app-id> --raw

# Get the digest plus the raw sample as JSON (for scripts and LLMs)
appsignal-cli samples show --incident 42 --app-id <app-id> --output json

# List the samples for an incident within a time window
appsignal-cli samples list --incident 42 --app-id <app-id> --start "2026-05-19T00:00:00Z" --end "2026-05-20T00:00:00Z"

# Scan a time window across incidents (no --incident) — "what happened between T1 and T2"
appsignal-cli samples list --app-id <app-id> --start "2026-05-19T13:00:00Z" --end "2026-05-19T14:00:00Z"

# Narrow a window scan to namespaces and a specific user
appsignal-cli samples list --app-id <app-id> --start "2026-05-19T13:00:00Z" --end "2026-05-19T14:00:00Z" --namespaces web --user alice@example.com

# Re-inspect samples you fetched earlier, offline — no API call
appsignal-cli samples cache list
appsignal-cli samples cache search "PG::QueryCanceled"

# Skip caching for a single fetch, or clear the cache
appsignal-cli samples show --incident 42 --app-id <app-id> --no-cache
appsignal-cli samples cache clear
```

By default `samples show` prints a **digest** — request overview, who hit it, a
performance breakdown by event group, the slowest events and queries, N+1
detection, and (for errors) the exception, backtrace, causes, and breadcrumbs.
`--output json` returns both the raw `sample` and a structured `analysis`
object; `--raw` prints the unprocessed sample instead of the digest.

Every sample fetched by `samples show`/`samples list` is also written to a local
cache (under the platform cache directory) so you can re-inspect or search it
offline with `samples cache list` / `samples cache search` — handy when an
incident is closed or you have lost network access. Caching is best-effort and
never blocks a fetch. Because samples can include request parameters and session
data, the cache may hold sensitive values; disable it per call with `--no-cache`
or globally with `APPSIGNAL_NO_CACHE=1`, and wipe it with `samples cache clear`.

### Metrics

```sh
# Discover the metric keys reported by an app
appsignal-cli metrics list --app-id <app-id>

# Filter the key list by name fragment
appsignal-cli metrics list --app-id <app-id> --name database

# Pull a metric's values over a relative window
appsignal-cli metrics timeseries --app-id <app-id> --metric database.query_count --timeframe R1H

# Pull a metric over an explicit window, narrowed to a field and a tag
appsignal-cli metrics timeseries --app-id <app-id> --metric latency \
  --field p95 --tag hostname=web-1 \
  --start "2026-05-19T13:00:00Z" --end "2026-05-19T14:00:00Z"

# Error and performance throughput per action over a window ("what got slow / noisy")
appsignal-cli metrics history --app-id <app-id> \
  --start "2026-05-19T00:00:00Z" --end "2026-05-20T00:00:00Z" --namespaces web
```

Metrics come from the public GraphQL API (`app.metrics.keys` /
`app.metrics.timeseries` and the `timeDetective*DataPoints` fields) — no REST
access or extra credentials are needed. `metrics timeseries` requires a window:
either `--timeframe` (e.g. `R1H`, `R1D`) or both `--start` and `--end`.

### Performance

```sh
# Rank recent performance actions by mean request duration (the slowest typical request)
appsignal-cli performance actions --app-id <app-id>

# Rank by total time spent or by throughput instead
appsignal-cli performance actions --app-id <app-id> --sort total
appsignal-cli performance actions --app-id <app-id> --sort count --namespaces web

# Drill into the slowest actions and show the slow queries + N+1 suspects behind them
appsignal-cli performance queries --app-id <app-id> --limit 5
```

`performance actions` ranks the recent performance incidents by `mean` (default),
`total`, or `count`. `performance queries` takes the slowest actions, fetches the
latest sample for each, and surfaces the slow database queries and N+1 suspects
from those samples — useful for "why is this action slow?". Because the public
API exposes performance data at the **action** level (not per-SQL), the query
view is derived from the latest sampled request per action, not a full aggregate;
the output says so.

### Logs

```sh
# Search logs with a time range
appsignal-cli logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --end "2025-03-16T07:00:00Z" \
  --query 'group=notifiers message:"[Email]"'

# Fetch all logs in a time range (auto-paginate)
appsignal-cli --output json logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --query "group:notifiers" --page-all
```

### Triggers

```sh
# List anomaly detection triggers
appsignal-cli triggers list --app "MyApp" --environment "production"

# Create a trigger
appsignal-cli triggers create --app "MyApp" --environment "production" \
  --name "Slow web requests" \
  --metric-name response_time --kind Advanced --field mean \
  --comparison-operator ">" --condition-value 500 \
  --description "Alert when mean response time stays above 500ms" \
  --warmup-duration 5 --cooldown-duration 2
```

### LLM skills

```sh
# Install the bundled AppSignal LLM skill for OpenCode-style agents
appsignal-cli skill install

# Install for Codex
appsignal-cli skill install --target codex

# Install for Claude user skills
appsignal-cli skill install --target claude
```

## Commands

### `about`

| Command | Description |
|---|---|
| `about` | Show the CLI overview screen with version, config, auth, and starter commands |

### `auth`

| Command | Description |
|---|---|
| `auth login [--org SLUG]` | Authenticate via OAuth using the active config for the current project or your global config |
| `auth logout` | Remove stored credentials from the active config |
| `auth status` | Show authentication status and expiry |

### `project`

| Command | Description |
|---|---|
| `project init [--org SLUG]` | Create or update the project-local `.appsignal.toml`, which becomes the only config used in that project |

### `apps`

| Command | Description |
|---|---|
| `apps list` | List apps for the current OAuth account and save the default org to the active config |
| `apps info --app-id <id>` | Show details for a specific app |
| `apps find --name <name> [--environment <env>]` | Find an app by name |
| `apps resources all` | Show all supported app resources in one go |
| `apps resources users` | Show app users |
| `apps resources notifiers` | Show app notifiers |
| `apps resources namespaces` | Show app namespaces |
| `apps resources dashboards` | Show app dashboards |
| `apps resources deploy-markers` | Show recent deploy markers |
| `apps set-org --org <slug>` | Set the default organization in the active config |
| `apps show-org` | Show the current default organization |

### `incidents`

| Command | Description |
|---|---|
| `incidents list` | List all incident types for an app |
| `incidents list-exceptions` | List exception incidents (supports text search) |
| `incidents list-performance` | List performance incidents (supports text search) |
| `incidents list-anomalies` | List anomaly detection incidents |
| `incidents show --number <N>` | Show details for a specific incident |
| `incidents update --number <N[,N...]>` | Update incident state, severity, or assignees; multiple numbers currently support `--state` only |
| `incidents add-note --number <N> --content "..."` | Add a note to an incident |

### `samples`

| Command | Description |
|---|---|
| `samples show [URL\|id]` | Show an analysed digest of one sample — the latest, or `--sample-id <id>`, or `--at <ISO>` (closest to a timestamp); `--raw` for the unprocessed sample |
| `samples list [URL]` | List an incident's samples, or — with no `--incident` — scan a time window (`--start`/`--end`) across incidents, filterable by `--namespaces` and `--user` |
| `samples cache list` | List recently cached samples (newest first); filter with `--app-id`, cap with `--limit` |
| `samples cache search <query>` | Search cached samples by their contents (action, user, query bodies, exceptions, …) |
| `samples cache clear` | Delete every cached sample |
| `samples cache path` | Print the cache directory |

Both `show` and `list` accept an AppSignal incident or sample URL (or a bare sample id) as a positional argument, or the explicit `--incident <N>` plus the usual `--app-id`/`--app`/`--environment`/`--org` flags. In window mode `--limit` caps how many recent incidents are scanned (default 20).

### `metrics`

| Command | Description |
|---|---|
| `metrics list` | Discover the metric keys reported by an app; filter with `--name <fragment>` and cap with `--limit` |
| `metrics timeseries --metric <name>` | Fetch a metric's values over time; narrow with `--field` and `--tag key=value`; window with `--timeframe` (e.g. `R1H`) or `--start`/`--end` |
| `metrics history --start <ISO> --end <ISO>` | Per-action error and performance throughput over a window; scope with `--namespaces` |

All `metrics` subcommands take the usual `--app-id`/`--app`/`--environment`/`--org` flags and are served by the public GraphQL API.

### `performance`

| Command | Description |
|---|---|
| `performance actions` | Rank recent performance incidents by `--sort mean\|total\|count` (default `mean`); scan depth via `--limit`, scope with `--namespaces`/`--action`/`--state` |
| `performance queries` | Drill into the `--limit` slowest actions and list the slow queries and N+1 suspects from each one's latest sample |

Both take the usual `--app-id`/`--app`/`--environment`/`--org` flags. `performance queries` is sample-derived (latest request per action), not a full per-query aggregate.

### `logs`

| Command | Description |
|---|---|
| `logs tail` | Stream log lines in real time (polls every second) |
| `logs search` | Search log lines (one-shot query, supports global `--output json` or `--format json` for LLM use) |
| `logs views` | List saved log views (filter presets) |
| `logs sources` | List log sources for an app |
| `logs metrics list` | List log-derived metrics |
| `logs metrics create` | Create a log-derived metric |
| `logs metrics update --id <id>` | Update a log-derived metric |
| `logs metrics delete --id <id>` | Delete a log-derived metric |
| `logs triggers list` | List log-based triggers |
| `logs triggers create` | Create a log-based trigger |
| `logs triggers update --id <id>` | Update a log-based trigger |
| `logs triggers delete --id <id>` | Delete a log-based trigger |

### `dashboards`

| Command | Description |
|---|---|
| `dashboards list` | List dashboards for an app |
| `dashboards create` | Create a dashboard |
| `dashboards update --id <id>` | Update a dashboard |

### `triggers`

| Command | Description |
|---|---|
| `triggers list` | List anomaly detection triggers for an app |
| `triggers create` | Create a new anomaly detection trigger |
| `triggers update --id <id>` | Update a trigger by creating a new version |
| `triggers archive --id <id>` | Archive a trigger |

#### Trigger naming options

| Flag | Description |
|---|---|
| `--name <text>` | Human-readable trigger name shown in alerts and lists |
| `--metric-name <metric>` | Actual metric the trigger monitors |
| `--description <text>` | Optional longer description/instructions for the trigger |

### `skill`

| Command | Description |
|---|---|
| `skill install` | Install the bundled AppSignal LLM skill for one or more supported agent targets |
| `skill update` | Update an installed AppSignal LLM skill to the bundled version |
| `skill status` | Show whether an installed AppSignal LLM skill is current, outdated, missing, or unversioned |

Targets:
- `opencode` (default): `~/.agents/skills/appsignal/SKILL.md`
- `codex`: `$CODEX_HOME/skills/appsignal/SKILL.md` or `~/.codex/skills/appsignal/SKILL.md`
- `claude`: `~/.claude/skills/appsignal/SKILL.md`
- `all`: install all of the above

Use `skill install --target codex`, `skill install --target claude`, or `skill install --target all` to choose a target. Use `skill install --dir <path>` to install into a custom skills root for a single target, or `skill install --force` to overwrite an existing install.

Installed skills include the CLI version that produced them. `skill status` checks all supported targets by default so you get one list of every provider status, and `skill update` refreshes an existing install after upgrading `appsignal-cli`.

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
| `--assign-me` | Assign the incident to the authenticated CLI user |
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
| `--page-all` | Auto-paginate to fetch all results in the time range |

Global output flag for any command:

| Flag | Description |
|---|---|
| `--output <human|json>` | Render command results for people or machines |
| `--format <human|json>` | Synonym for `--output` |

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
appsignal-cli --output json logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --query 'group=notifiers message:"[Email]"' --page-all

# Get JSON output for LLM consumption
appsignal-cli --format json logs search --app "MyApp" --environment "production" --query "error"

# List available log views
appsignal-cli logs views --app "MyApp" --environment "production"

# List log sources
appsignal-cli logs sources --app "MyApp" --environment "production"

# Create a log-derived metric from matching log lines
appsignal-cli logs metrics create --app "MyApp" --environment "production" \
  --name "Track error count" \
  --query 'severity:error' \
  --metric 'name=log.error_count,type=counter'

# Create a log-based trigger for matching log lines
appsignal-cli logs triggers create --app "MyApp" --environment "production" \
  --name "Root login" \
  --query 'message:root' \
  --severity ERROR \
  --notifier-id notifier_123
```

## Configuration

Config is stored globally at `~/.config/appsignal/config.toml`.

You can also add a project-local `.appsignal.toml` anywhere in your project. When
the CLI runs inside that project (or a subdirectory), it uses the nearest
`.appsignal.toml` as the only config for that project.

The easiest way to create one is:

```sh
appsignal-cli project init
```

If you run that command inside a git checkout, the CLI creates or updates
`.appsignal.toml` at the repository root. Outside git, it uses the current
directory.

Do not commit `.appsignal.toml` or any config file that contains OAuth tokens.
This repository ignores `.appsignal.toml`; add the same rule to your own project
if you use project-local credentials.

Example:

```toml
# .appsignal.toml
org = "your-org-slug"

[oauth]
access_token = "..."
refresh_token = "..."
expires_at = 1742324400
```

When a local project config is active, commands read and write only that
`.appsignal.toml` file. Otherwise they use the global config.

Global config example:

```toml
org = "your-org-slug"

# Set automatically by `auth login`:
[oauth]
access_token = "..."
refresh_token = "..."
expires_at = 1742324400
```

Expired OAuth tokens are automatically refreshed before API calls.
OAuth always uses the built-in local callback at `http://127.0.0.1:9789/callback`.

The `org` value is saved automatically when you run `apps list` or
`apps set-org --org <slug>` into whichever config is active. Use `project init`
first if you want those writes to stay local to the project.

## Releases

Releases are published by AppSignal maintainers through GitHub Actions. Release
artifacts are available on the
[GitHub releases page](https://github.com/appsignal/appsignal-cli/releases).

## Development

You need a stable Rust toolchain and Cargo to work on this project.

```sh
# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check
```

CI runs all three checks on every push and pull request via GitHub Actions.

### Custom endpoints

The CLI supports custom AppSignal endpoints for AppSignal development and
testing. Most users should not need these options.

```sh
appsignal-cli project init \
  --endpoint https://appsignal.example.com \
  --rest-endpoint https://public-api.appsignal.example.com \
  --oauth-client-id your-oauth-client-id \
  --org your-org-slug
```

`endpoint` must be the base URL, without `/graphql`. When set to a base URL like
`https://appsignal.example.com`, the CLI uses `/graphql` for API calls and the
base URL itself for OAuth. Values like `https://appsignal.example.com/graphql`
are not supported.

`rest_endpoint` can override the REST/public API base URL separately. When
`oauth_client_id` is unset, the CLI uses the production OAuth client ID. For
custom endpoints that advertise an OAuth `registration_endpoint`, the CLI
automatically registers a public client and uses the returned `client_id`
instead.

### Versioning

This project uses [Semantic Versioning][semver].

The `main` branch is the primary development branch. Open pull requests against
`main` unless a maintainer asks you to target another branch.

Every stable and unstable release is tagged in git with a version tag.

### Changesets

This project uses changesets, as managed by [mono], to update the changelog and
trigger new releases. Every meaningful change that needs a release requires a
changeset. Follow the guide on the [mono] project page on how to create one.

## Contributing

Thinking of contributing to this project? Thank you.

Before opening a pull request:

- Keep changes focused and include tests when changing behavior
- Run `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`
- Add a changeset for user-facing changes
- Avoid including private AppSignal app data, logs, tokens, or customer details
  in issues, pull requests, tests, or fixtures

Please follow our [Contributing guide][contributing-guide] in our
documentation and follow our [Code of Conduct][coc].

Also, we would be very happy to send you Stroopwafles. Have a look at everyone
we have sent a package to so far on our [Stroopwafles page][waffles-page].

## Support

[Contact us][contact] and speak directly with the engineers working on
AppSignal. They will help you get set up, tweak your code and make sure you get
the most out of using AppSignal.

Also see our [SUPPORT.md file](SUPPORT.md).

## License

This project is released under the MIT license. See [LICENSE](LICENSE).

[appsignal]: https://www.appsignal.com/
[contact]: mailto:support@appsignal.com
[coc]: https://docs.appsignal.com/appsignal/code-of-conduct.html
[contributing-guide]: https://docs.appsignal.com/appsignal/contributing.html
[waffles-page]: https://www.appsignal.com/waffles
[docs]: https://docs.appsignal.com
[mono]: https://github.com/appsignal/mono/
[semver]: https://semver.org/
