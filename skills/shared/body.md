Use this skill when the user wants to inspect AppSignal data through `appsignal-cli`.

## Working Rules

1. Prefer `appsignal-cli <command> --help` when you need exact flag syntax.
2. Use `logs search --json` when the result needs to be parsed by an LLM or script.
3. Use `apps list --org <slug>` once to save a default organization before name-based app lookups.
4. Prefer `--app-id` when known; otherwise use `--app` and `--environment` together for unambiguous app resolution.
5. Quote names, environments, queries, and note content when they contain spaces or special characters.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login --oauth` | Authenticate via OAuth |
| `appsignal-cli auth login [--token TOKEN]` | Store a personal API token |
| `appsignal-cli auth logout` | Remove stored credentials |
| `appsignal-cli auth status` | Show the current authentication status |
| `appsignal-cli apps orgs` | List organizations you have access to |
| `appsignal-cli apps list --org <slug>` | List apps in an organization and save the org as default |
| `appsignal-cli apps info --app-id <id>` | Show details for an app by ID |
| `appsignal-cli apps find --name <name> [--environment <env>] [--org <slug>]` | Find an app by name |
| `appsignal-cli apps set-org --org <slug>` | Set the default organization |
| `appsignal-cli apps show-org` | Show the current default organization |
| `appsignal-cli incidents list [options]` | List all incident types for an app |
| `appsignal-cli incidents list-exceptions [options]` | List exception incidents |
| `appsignal-cli incidents list-performance [options]` | List performance incidents |
| `appsignal-cli incidents list-anomalies [options]` | List anomaly incidents |
| `appsignal-cli incidents show --number <N> [app options]` | Show details for a single incident |
| `appsignal-cli incidents update --number <N> [flags]` | Update state, severity, assignees, or description |
| `appsignal-cli incidents add-note --number <N> --content "..."` | Add a note to an incident |
| `appsignal-cli logs tail [filters]` | Stream log lines in real time |
| `appsignal-cli logs search [filters] [--json] [--page-all]` | Search log lines once |
| `appsignal-cli logs views [app options]` | List saved log views |
| `appsignal-cli logs sources [app options]` | List log sources |
| `appsignal-cli skill install [--target TARGET] [--dir PATH] [--force]` | Install the bundled AppSignal skill |

## App Selection

Most app-specific commands accept either:

- `--app-id <id>`
- `--app <name> [--environment <env>]`

Use `--app-id` when available. Use `--environment` when the same app name exists in multiple environments.

## Incident Options

Common flags for `incidents list`, `list-exceptions`, `list-performance`, and `list-anomalies`:

| Flag | Description |
|---|---|
| `--app <name>` | App name |
| `--environment <env>` | Environment filter |
| `--app-id <id>` | App ID |
| `--org <slug>` | Organization; defaults to the saved org |
| `--limit <N>` | Maximum number of results |
| `--offset <N>` | Pagination offset |
| `--state <OPEN|CLOSED|WIP>` | Incident state filter |
| `--order <LAST|ID>` | Sort order |

Additional incident flags:

| Flag | Description |
|---|---|
| `--namespaces <ns>` | Comma-separated namespaces |
| `--action <name>` | Action name filter |
| `--query <text>` | Text search for exception and performance incidents |

Useful `incidents update` flags:

| Flag | Description |
|---|---|
| `--state <OPEN|CLOSED|WIP>` | Change state |
| `--severity <...>` | Change severity |
| `--assign <id,id>` | Assign users |
| `--description "..."` | Update description |

## Log Options

Shared flags for `logs tail` and `logs search`:

| Flag | Description |
|---|---|
| `--query <text>` | Log query syntax |
| `--severities <list>` | Comma-separated severities |
| `--source-ids <list>` | Comma-separated source IDs |
| `--view <name-or-id>` | Apply a saved log view |

Extra `logs search` flags:

| Flag | Description |
|---|---|
| `--start <ISO8601>` | Start time |
| `--end <ISO8601>` | End time |
| `--limit <N>` | Maximum results |
| `--order <ASC|DESC>` | Sort order |
| `--json` | Machine-readable JSON output |
| `--page-all` | Auto-paginate to fetch all results |

## Log Query Syntax

Common query forms:

| Form | Meaning | Example |
|---|---|---|
| `field=value` | Exact match | `severity=error` |
| `field!=value` | Not equal | `source!=mongodb` |
| `field:value` | Contains | `message:timeout` |
| `field!:value` | Does not contain | `hostname!:test` |
| `field>value` | Numeric comparison | `duration>100` |

Notes:

- Space-separated terms act like `AND`.
- Use `OR` for alternatives.
- Use quotes for spaces or special characters: `message:"[Email]"`.
- Prefer `--severities` over embedding severity filters in the query.

## Skill Install Targets

| Target | Install location |
|---|---|
| `opencode` | `~/.agents/skills/appsignal/SKILL.md` |
| `codex` | `$CODEX_HOME/skills/appsignal/SKILL.md` or `~/.codex/skills/appsignal/SKILL.md` |
| `claude` | `~/.claude/skills/appsignal/SKILL.md` |
| `all` | Install all supported targets |

## Examples

Authenticate:

```bash
appsignal-cli auth login --oauth
```

Save the default org and list apps:

```bash
appsignal-cli apps list --org my-org
```

Find an app by name:

```bash
appsignal-cli apps find --name "MyApp" --environment "production"
```

List the latest incidents:

```bash
appsignal-cli incidents list --app "MyApp" --environment "production" --limit 10 --order LAST
```

Show a single incident:

```bash
appsignal-cli incidents show --number 42 --app "MyApp" --environment "production"
```

Close an incident:

```bash
appsignal-cli incidents update --number 42 --app "MyApp" --environment "production" --state CLOSED
```

Add an incident note:

```bash
appsignal-cli incidents add-note --number 42 --app "MyApp" --environment "production" --content "Investigated and resolved."
```

Search logs with JSON output:

```bash
appsignal-cli logs search --app "MyApp" --environment "production" --query "timeout" --json
```

Search all matching logs in a time window:

```bash
appsignal-cli logs search --app "MyApp" --environment "production" --start "2025-03-16T06:00:00Z" --query "group:notifiers" --page-all --json
```

Tail logs using a saved view:

```bash
appsignal-cli logs tail --app "MyApp" --environment "production" --view "Error logs"
```

Install the skill for Codex:

```bash
appsignal-cli skill install --target codex
```
