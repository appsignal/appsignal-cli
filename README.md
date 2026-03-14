# appsignal-cli

A command-line interface for [AppSignal](https://appsignal.com), built in Rust. Designed to be used by both humans and LLMs to query AppSignal data from the terminal.

## Installation

```sh
cargo install --path .
```

Requires Rust 1.70+.

## Authentication

Get your personal API token from https://appsignal.com/users/edit, then:

```sh
appsignal-cli auth login --token <your-token>
```

The token is stored in `~/.config/appsignal/config.toml`.

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

# List anomaly detection alerts
appsignal-cli incidents list-anomalies --app "MyApp" --environment "production"

# Show details for a specific incident
appsignal-cli incidents show --number 42 --app "MyApp" --environment "production"
```

## Commands

### `auth`

| Command | Description |
|---|---|
| `auth login [--token TOKEN]` | Store API token (prompts if omitted) |
| `auth logout` | Remove stored credentials |
| `auth status` | Show authentication status |

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
| `incidents list-anomalies` | List anomaly detection incidents |
| `incidents show --number <N>` | Show details for a specific incident |

All incident commands accept either `--app-id <id>` or `--app <name> [--environment <env>]` to identify the application. The `--environment` flag is needed when multiple apps share the same name.

#### Common options

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

#### Additional options for `list` and `list-exceptions`

| Flag | Description |
|---|---|
| `--namespaces <ns>` | Filter by namespaces (comma-separated, e.g. "web,background") |
| `--action <name>` | Filter by action name (e.g. "UsersController#show") |

#### Additional option for `list-exceptions`

| Flag | Description |
|---|---|
| `--query <text>` | Search exception name or message |

## Configuration

Config is stored at `~/.config/appsignal/config.toml`:

```toml
token = "your-api-token"
org = "your-org-slug"
```

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
