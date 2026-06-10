# appsignal-cli

A command-line interface for [AppSignal](https://appsignal.com), built in Rust. Designed to be used by both humans and LLMs to query AppSignal data from the terminal.

- [AppSignal.com website][appsignal]
- [Documentation][docs]
- [Support][contact]

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

For project-specific setup, initialize `.appsignal.toml` first:

```sh
appsignal-cli project init
appsignal-cli auth login --oauth
```

You can also set a project-specific endpoint, OAuth client ID, and default org
during initialization:

```sh
appsignal-cli project init \
  --endpoint https://staging.lol \
  --oauth-client-id your-staging-client-id \
  --org my-sideproject
```

### Personal API token

Get your personal API token from https://appsignal.com/users/edit, then:

```sh
appsignal-cli auth login --token <your-token>
```

Credentials are stored in `~/.config/appsignal/config.toml` by default. Once a
project-local `.appsignal.toml` exists, commands run in that project use it
automatically.

`project init` does not copy your stored global token or OAuth credentials into
the local file. Authenticate afterward if you want project-specific credentials.

## Quick start

```sh
# List your organizations
appsignal-cli apps orgs

# List apps in an organization (saves the org as default)
appsignal-cli apps list --org <org-slug>

# Initialize a project-local config
appsignal-cli project init --org <org-slug>

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

# List anomaly detection triggers
appsignal-cli triggers list --app "MyApp" --environment "production"

# Create a trigger
appsignal-cli triggers create --app "MyApp" --environment "production" \
  --name "Slow web requests" \
  --metric-name response_time --kind Advanced --field mean \
  --comparison-operator ">" --condition-value 500 \
  --description "Alert when mean response time stays above 500ms" \
  --warmup-duration 5 --cooldown-duration 2

# Search logs with JSON output (for LLMs)
appsignal-cli --format json logs search --app "MyApp" --environment "production" --query "timeout"

# Install the bundled AppSignal LLM skill for OpenCode-style agents
appsignal-cli skill install

# Install for Codex
appsignal-cli skill install --target codex

# Install for Claude user skills
appsignal-cli skill install --target claude

# Fetch all logs in a time range (auto-paginate)
appsignal-cli --output json logs search --app "MyApp" --environment "production" \
  --start "2025-03-16T06:00:00Z" --query "group:notifiers" --page-all
```

## Commands

### `about`

| Command | Description |
|---|---|
| `about` | Show the CLI overview screen with version, config, auth, and starter commands |

### `auth`

| Command | Description |
|---|---|
| `auth login --oauth [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Authenticate via OAuth using the active config for the current project or your global config |
| `auth login [--token TOKEN] [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Authenticate with a personal API token using the active config for the current project or your global config |
| `auth logout` | Remove stored credentials from the active config |
| `auth status` | Show authentication status and method |

### `project`

| Command | Description |
|---|---|
| `project init [--endpoint URL] [--oauth-client-id ID] [--org SLUG]` | Create or update the project-local `.appsignal.toml`, which becomes the only config used in that project |

### `apps`

| Command | Description |
|---|---|
| `apps orgs` | List all organizations you have access to |
| `apps list --org <slug>` | List apps in an organization and save the default org to the active config |
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
| `incidents update --number <N>` | Update incident state, severity, or assignees |
| `incidents add-note --number <N> --content "..."` | Add a note to an incident |

### `logs`

| Command | Description |
|---|---|
| `logs tail` | Stream log lines in real time (polls every second) |
| `logs search` | Search log lines (one-shot query, supports global `--output json` or `--format json` for LLM use) |
| `logs views` | List saved log views (filter presets) |
| `logs sources` | List log sources for an app |

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

Example:

```toml
# .appsignal.toml
endpoint = "https://staging.lol"
oauth_client_id = "your-staging-client-id"

[oauth]
access_token = "..."
refresh_token = "..."
expires_at = 1742324400
```

When a local project config is active, commands read and write only that
`.appsignal.toml` file. Otherwise they use the global config.

Global config example:

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
For custom endpoints that advertise an OAuth `registration_endpoint`, the CLI
automatically registers a public client and uses the returned `client_id`
instead.
When `endpoint` is set to a base URL like `https://staging.lol`, the CLI uses
`/graphql` for API calls and the base URL itself for OAuth. Values like
`https://staging.lol/graphql` are not supported.
OAuth always uses the built-in local callback at `http://127.0.0.1:9789/callback`.

The `org` value is saved automatically when you run `apps list --org <slug>` or `apps set-org --org <slug>` into whichever config is active. Use `project init` first if you want those writes to stay local to the project.

## Releases

Follow the process below to release a new version of this project.

1. On GitHub open the Actions tab.
2. Select the "Publish a release" workflow.
3. Click the "Run workflow" button, select a different branch if necessary, but
   `main` is often the branch to release.
4. Then press run the green "Run workflow" button.

This will trigger a GitHub workflow to compile and release the project
automatically.
Keep an eye on the workflow in case it fails.

This process also triggers a changelog Pull Request to be created on the
appsignal.com repository for the public changelog.
You will be assigned to this Pull Request.
Make sure that also gets merged.

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

### Versioning

This gem uses [Semantic Versioning][semver].

The `main` branch corresponds to the current stable release of the gem.

The `develop` branch is used for development of features that will end up in
the next minor release, if present.

Open a Pull Request on the `main` branch if you're fixing a bug. For new
features, open a Pull Request on the `develop` branch.

Every stable and unstable release is tagged in git with a version tag.

### Changesets

This project uses changesets, as managed by [mono], to update the changelog and
trigger new releases. Every meaningful change that needs a release requires a
changeset. Follow the guide on the [mono] project page on how to create one.

## Contributing

Thinking of contributing to this project? Awesome! 🚀

Please follow our [Contributing guide][contributing-guide] in our
documentation and follow our [Code of Conduct][coc].

Also, we would be very happy to send you Stroopwafles. Have look at everyone
we send a package to so far on our [Stroopwafles page][waffles-page].

## Support

[Contact us][contact] and speak directly with the engineers working on
AppSignal. They will help you get set up, tweak your code and make sure you get
the most out of using AppSignal.

Also see our [SUPPORT.md file](SUPPORT.md).

[appsignal]: https://www.appsignal.com/
[appsignal-sign-up]: https://appsignal.com/users/sign_up
[contact]: mailto:support@appsignal.com
[coc]: https://docs.appsignal.com/appsignal/code-of-conduct.html
[contributing-guide]: https://docs.appsignal.com/appsignal/contributing.html
[waffles-page]: https://www.appsignal.com/waffles
[docs]: https://docs.appsignal.com
[mono]: https://github.com/appsignal/mono/
