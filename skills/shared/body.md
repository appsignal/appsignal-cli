Use this skill when the user wants to inspect AppSignal data through `appsignal-cli`.

## Working Rules

1. Prefer `appsignal-cli <command> --help` when you need exact flag syntax.
2. Use the global `--output json` flag when the result needs to be parsed by an LLM or script.
3. Use `apps list` once to save a default organization before name-based app lookups.
4. Prefer `--app-id` when known; otherwise use `--app` and `--environment` together for unambiguous app resolution.
5. Quote names, environments, queries, and note content when they contain spaces or special characters.

## Commands

| Command | Description |
|---|---|
| `appsignal-cli auth login` | Authenticate via OAuth |
| `appsignal-cli auth logout` | Remove stored credentials |
| `appsignal-cli auth status` | Show the current authentication status |
| `appsignal-cli apps list` | List apps for the current OAuth account and save the org as default |
| `appsignal-cli apps info --app-id <id>` | Show details for an app by ID |
| `appsignal-cli apps find --name <name> [--environment <env>] [--org <slug>]` | Find an app by name |
| `appsignal-cli apps resources all [app options]` | Show all supported app resources |
| `appsignal-cli apps resources deploy-markers [app options]` | Show recent deploy markers |
| `appsignal-cli apps set-org --org <slug>` | Set the default organization |
| `appsignal-cli apps show-org` | Show the current default organization |
| `appsignal-cli incidents list [options]` | List all incident types for an app |
| `appsignal-cli incidents list-exceptions [options]` | List exception incidents |
| `appsignal-cli incidents list-performance [options]` | List performance incidents |
| `appsignal-cli incidents list-anomalies [options]` | List anomaly incidents |
| `appsignal-cli incidents show --number <N> [app options]` | Show details for a single incident |
| `appsignal-cli incidents update --number <N[,N...]> [flags]` | Update state, severity, notification frequency, assignees, or description; multiple numbers currently support `--state` only |
| `appsignal-cli incidents add-note --number <N> --content "..."` | Add a Markdown-formatted note to an incident |
| `appsignal-cli incidents list-notes --number <N>` | List incident notes with IDs, authors, sources, permissions, and timestamps |
| `appsignal-cli incidents update-note --number <N> --id <ID> --content "..."` | Update one of your incident notes |
| `appsignal-cli incidents delete-note --number <N> --id <ID>` | Delete one of your incident notes |
| `appsignal-cli samples incident --number <N> [app options]` | List performance samples/traces for a performance incident |
| `appsignal-cli samples list --namespace <ns> --action <name> [app options]` | List performance samples/traces for a known namespace/action |
| `appsignal-cli samples errors --digest <digest> [app options]` | List error traces for an exception digest |
| `appsignal-cli samples show --namespace <ns> --action <name> --trace-id <id> [app options]` | Show a sample/trace span tree or span details |
| `appsignal-cli samples show-error --digest <digest> --trace-id <id> [app options]` | Show an error trace span tree or span details |
| `appsignal-cli samples show-incident --number <N> --trace-id <id> [app options]` | Show a trace from a performance or exception incident without passing namespace/action/digest |
| `appsignal-cli logs tail [filters]` | Stream log lines in real time |
| `appsignal-cli logs search [filters] [--page-all]` | Search log lines once |
| `appsignal-cli logs views [app options]` | List saved log views |
| `appsignal-cli logs sources [app options]` | List log sources |
| `appsignal-cli logs metrics <list|create|update|delete> ...` | Manage log-derived metric configurations |
| `appsignal-cli logs triggers <list|create|update|delete> ...` | Manage log-based trigger configurations |
| `appsignal-cli triggers list [app options] [filters]` | List anomaly detection triggers |
| `appsignal-cli triggers create [app options] [definition flags]` | Create an anomaly detection trigger |
| `appsignal-cli triggers update --id <id> [app options] [definition flags]` | Update a trigger by creating a new version |
| `appsignal-cli triggers archive --id <id> [app options]` | Archive a trigger |
| `appsignal-cli feedback [MESSAGE] [--email <email>] [--no-email]` | Send CLI feedback about missing endpoints, missing features, or broken behavior |
| `appsignal-cli skill install [--target TARGET] [--dir PATH] [--force]` | Install the bundled AppSignal skill |
| `appsignal-cli skill update [--target TARGET] [--dir PATH]` | Update an installed AppSignal skill to the bundled version |
| `appsignal-cli skill status [--target TARGET] [--dir PATH]` | Show whether installed AppSignal skills are current, outdated, missing, or unversioned; defaults to all supported targets |

## App Selection

Most app-specific commands accept either:

- `--app-id <id>`
- `--app <name> [--environment <env>]`

Use `--app-id` when available. Use `--environment` when the same app name exists in multiple environments.

## CLI Feedback

Use `appsignal-cli feedback` when AppSignal CLI is missing an endpoint, missing a feature, or not working as expected. Pass short feedback as an argument, or pipe a longer multi-line report on stdin.

```bash
appsignal-cli feedback "Please add support for uptime monitor endpoints" --email ada@example.com
```

```bash
appsignal-cli --output json feedback --email ada@example.com <<'EOF'
The logs search command needs a way to export all fields without truncation.
This is important for LLM workflows that analyze structured log attributes.
EOF
```

The optional `--email` value is stored in the active AppSignal config for future feedback. Use `--no-email` to omit contact details.

## AppSignal URL Structure

If you already have an AppSignal URL, you can often reuse parts of it in the CLI.

Useful parts of common URLs:

| URL pattern | Meaning | CLI mapping |
|---|---|---|
| `https://appsignal.com/<org-slug>` | Organization/account home | `apps list` or `apps set-org --org <org-slug>` |
| `https://appsignal.com/<account-slug>/sites/<site_id>` | App root | Use `--app-id <site_id>` |
| `https://appsignal.com/<account-slug>/sites/<site_id>/dashboard` | App dashboard | Use `--app-id <site_id>` |
| `https://appsignal.com/<account-slug>/sites/<site_id>/performance` | Performance area | Use `--app-id <site_id>` with `incidents list-performance` |
| `https://appsignal.com/<account-slug>/sites/<site_id>/exceptions` | Exceptions area | Use `--app-id <site_id>` with `incidents list-exceptions` |
| `https://appsignal.com/<account-slug>/sites/<site_id>/anomalies` | Anomalies area | Use `--app-id <site_id>` with `incidents list-anomalies` |
| `https://appsignal.com/<account-slug>/sites/<site_id>/logs` | Logs area | Use `--app-id <site_id>` with `logs ...` commands |
| `https://appsignal.com/<account-slug>/sites/<site_id>/triggers` | Trigger list | Use `--app-id <site_id>` with `triggers ...` commands |
| `https://appsignal.com/<account-slug>/sites/<site_id>/markers` | Deploy markers | Use `--app-id <site_id>` with `apps resources deploy-markers` |

Notes:

- In app URLs, `<site_id>` is the AppSignal app ID, so it maps directly to `--app-id`.
- The CLI usually does not need `<account-slug>`.

Incident URLs:

| URL pattern | CLI mapping |
|---|---|
| `.../sites/<site_id>/incidents/<number>` | `incidents show --app-id <site_id> --number <number>` |
| `.../sites/<site_id>/performance/incidents/<number>` | `incidents show --app-id <site_id> --number <number>` |
| `.../sites/<site_id>/exceptions/incidents/<number>` | `incidents show --app-id <site_id> --number <number>` |
| `.../sites/<site_id>/anomalies/incidents/<number>` | `incidents show --app-id <site_id> --number <number>` |
| `.../sites/<site_id>/logs/incidents/<number>` | `incidents show --app-id <site_id> --number <number>` |

The same incident number still works if the URL ends with extra page sections such as:

- `/summary`
- `/attributes`
- `/logbook`
- `/graphs`
- `/samples/...`
- `/alerts/...`
- `/lines`
- `/traces/...`

Performance sample/trace URLs:

| URL pattern | CLI mapping |
|---|---|
| `.../sites/<site_id>/performance/incidents/<number>/samples` | `samples incident --app-id <site_id> --number <number>` |
| `.../sites/<site_id>/performance/incidents/<number>/samples/<trace_id>` | `samples incident --app-id <site_id> --number <number>` first, then `samples show --app-id <site_id> --namespace <ns> --action <action> --trace-id <trace_id>` |
| `.../sites/<site_id>/performance/traces/<namespace>/<action_name>/traces` | `samples list --app-id <site_id> --namespace <namespace> --action <action_name>` |
| `.../sites/<site_id>/performance/traces/<namespace>/<action_name>/traces/<trace_id>` | `samples show --app-id <site_id> --namespace <namespace> --action <action_name> --trace-id <trace_id>` |

Important namespace note:

- `--environment` is the app environment used for app lookup, such as `production` or `development`.
- `--namespace` is the AppSignal action namespace, usually `web`, `background`, `rake`, `runner`, or `graphql`.
- If unsure, prefer `samples incident --number <N>` because it reads the correct namespace and action names from the performance incident.

Logs URLs:

| URL pattern | CLI mapping |
|---|---|
| `.../sites/<site_id>/logs/<view_id>` | `logs tail --app-id <site_id> --view <view_id>` or `logs search --app-id <site_id> --view <view_id>` |
| `.../sites/<site_id>/logs/sources/<id>` | Use `<id>` in `--source-ids <id>` |

Examples:

```bash
# Save the organization from the current OAuth account
appsignal-cli apps list

# App URL: https://appsignal.com/my-account/sites/12345
appsignal-cli incidents list --app-id 12345

# Incident URL: https://appsignal.com/my-account/sites/12345/exceptions/incidents/42/logbook
appsignal-cli incidents show --app-id 12345 --number 42

# Log view URL: https://appsignal.com/my-account/sites/12345/logs/error-view
appsignal-cli logs tail --app-id 12345 --view error-view
```

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
| `--notification-frequency <...>` | Change when notifications are sent, such as `FIRST_AFTER_CLOSE` |
| `--notification-threshold <N>` | Set the occurrence number for `NTH_IN_HOUR` or `NTH_IN_DAY` |
| `--assign <id,id>` | Assign users |
| `--assign-me` | Assign the incident to the current CLI user |
| `--description "..."` | Update description |

## Performance Samples And Traces

Use `samples` or `traces`; they are aliases. Prefer `samples` when following the UI terminology for sample-based performance or exception incidents. Prefer `traces` when thinking in OpenTelemetry terms. Both command names call the same REST tracing API.

Recommended workflow from an incident:

```bash
appsignal-cli --output json samples incident --app-id <site_id> --number <incident_number>
```

This fetches the incident and lists matching traces. For performance incidents it reads the namespace and action names. For exception incidents it reads the incident digests and fetches error traces. This is the safest option because namespace/action and digest values must match exactly.

If the incident has multiple actions, the command fetches samples for all actions. Use `--action` to narrow it:

```bash
appsignal-cli --output json samples incident --app-id <site_id> --number <incident_number> --action "UsersController#show"
```

Direct namespace/action workflow:

```bash
appsignal-cli --output json samples list --app-id <site_id> --namespace web --action "UsersController#show"
```

Use direct `samples list` when you already know the exact namespace/action, when investigating a slow action before starting from an incident, or when querying OpenTelemetry-style action traces.

Filter traces by transaction tags with a trace query. Prefix tag names with `tag.`; multiple expressions are combined with AND:

```bash
appsignal-cli --output json samples list --app-id <site_id> --namespace web --action "UsersController#show" --query "tag.region=eu-west"
```

The trace query supports exact (`=`), not-equal (`!=`), contains (`:`), and does-not-contain (`!:`) operators for tags and revisions. For example, `--query "tag.region=eu-west tag.customer_tier!=free"`.

Direct exception digest workflow:

```bash
appsignal-cli --output json samples errors --app-id <site_id> --digest <digest>
```

Fetch all pages instead of the first page of results:

```bash
appsignal-cli --output json samples incident --app-id <site_id> --number <incident_number> --page-all
```

Sample/trace pagination is cursor-based by trace time, not offset-based. Without `--page-all`, the CLI fetches a single page controlled by `--limit` and capped at 100. With `--page-all`, it walks pages newest-first using the last trace timestamp as the next cursor and deduplicates boundary traces.

Inspect a returned sample/trace:

```bash
appsignal-cli --output json samples show-incident --app-id <site_id> --number <incident_number> --trace-id <trace_id>
```

This is the preferred follow-up after `samples incident` because it uses the incident number to resolve performance namespace/action values or exception digests internally.

Directly inspect a performance sample/trace when namespace and action are already known:

```bash
appsignal-cli --output json samples show --app-id <site_id> --namespace web --action "UsersController#show" --trace-id <trace_id>
```

Inspect a returned error trace:

```bash
appsignal-cli --output json samples show-error --app-id <site_id> --digest <digest> --trace-id <trace_id>
```

Inspect a specific span inside a trace:

```bash
appsignal-cli --output json samples show --app-id <site_id> --namespace web --action "UsersController#show" --trace-id <trace_id> --span-id <span_id>
```

Useful sample/trace flags:

| Flag | Description |
|---|---|
| `--number <N>` | Performance or exception incident number for `samples incident` |
| `--digest <digest>` | Exception digest for direct error trace lookup |
| `--namespace <ns>` | Action namespace for direct list/show, such as `web` or `background` |
| `--action <name>` | Action name; required for direct list/show, optional filter for incident lookup |
| `--trace-id <id>` | Trace/sample ID returned by `samples list` or `samples incident` |
| `--span-id <id>` | Span ID to inspect inside a trace |
| `--start <ISO8601>` | Start time; defaults to 24 hours ago |
| `--end <ISO8601>` | End time; defaults to now |
| `--min-duration-ms <N>` | Only list samples/traces slower than this duration |
| `--query <expression>` | Filter listed traces by tags or revision, such as `tag.region=eu-west` |
| `--limit <N>` | Maximum samples/traces to return, capped at 100 |
| `--page-all` | Automatically paginate to fetch all samples/traces; ignores `--limit` |
| `--include-sensitive` | Include HTTP headers, request parameters, session data, and function parameters in span detail output |

Troubleshooting sample lookup:

- If `samples list` returns no traces, check whether `--namespace` is an AppSignal namespace (`web`, `background`, etc.) rather than an app environment (`production`, `development`).
- Use `apps resources namespaces --app-id <site_id>` to list available namespaces.
- Use `incidents list-performance --app-id <site_id> --query "ActionName"` to find the performance incident and exact namespace/action.
- Prefer `samples incident --number <N>` when starting from a performance or exception incident URL or incident number.

## Output

Use the global output flag on any command:

| Flag | Description |
|---|---|
| `--output <human|json>` | Render command results for people or machines |

Example:

```bash
appsignal-cli --output json incidents show --number 42 --app "MyApp" --environment "production"
```

## Dashboard charts

1. Find a dashboard with `dashboards list --app-id APP`, then inspect chart IDs
   and configuration with `--output json dashboards show --app-id APP --id DASH`.
2. Create with `dashboards add-visual --app-id APP --dashboard-id DASH --type
   timeseries --file chart.json` (or `--type number`). Use `--file -` for stdin.
3. Update with `dashboards update-visual --app-id APP --dashboard-id DASH --id
   CHART --file changes.json`. The existing chart determines the type.
4. Read back with `dashboards show` to verify the requested settings.

JSON contains camelCase settings only. A timeseries definition:

```json
{"title":"Latency","format":"duration","metrics":[{"name":"transaction_duration","fields":[{"field":"MEAN"}],"tags":[{"key":"namespace","value":"web"}]}]}
```

A number definition:

```json
{"title":"Requests","metric":{"name":"requests","field":"COUNT","aggregate":"SUM"}}
```

Use actual metric names for the selected app. Creation requires a title. A patch
such as `{"title":"New title"}` preserves omitted settings. Supplied `metrics`,
`metric`, and `layout` replace the entire field; include full nested definitions.
Layout requires integer `x`, `y`, `w`, `h`. Metric fields are `MEAN`, `P90`, `P95`,
`COUNT`, `GAUGE`, `COUNTER`; number aggregates are `MAX`, `MIN`, `AVG`, `SUM`,
`FIRST`, `LAST`.

Both types accept `description`, `format`, `formatInput`, and `layout`. Timeseries
also accepts `lineLabel`, `display` (`LINE`, `AREA`, `AREA_RELATIVE`),
`drawNullAsZero`, and `minYAxis`. Formats are `number`, `percent`, `duration`,
`throughput`, `size`; size units are `bit`, `byte`, `kilobit`, `kilobyte`, `megabyte`.
Use `null` to clear optional description, formatting, layout, line label, minimum
axis, or number metric values; use `[]` to clear timeseries metrics. Changing away
from `size` clears `formatInput` on the server. Empty patches and unknown fields
are rejected. Responses contain `dashboard.visuals`, including chart IDs and
settings. Inspect the dashboard after an ambiguous creation failure before retrying.

## Trigger Options

Useful `triggers list` flags:

| Flag | Description |
|---|---|
| `--metric-name <metric>` | Filter by the metric being monitored |
| `--kind <kind>` | Filter by trigger kind |
| `--tag key=value` | Filter by trigger tags; repeat or comma-separate |

Useful `triggers create` and `triggers update` flags:

| Flag | Description |
|---|---|
| `--name <text>` | Human-readable trigger name shown in alerts and lists |
| `--metric-name <metric>` | Actual metric the trigger monitors |
| `--kind <kind>` | Trigger kind such as `Advanced` or `HostCPUUsage` |
| `--field <count|counter|gauge|mean|p90|p95>` | Metric field used for comparisons |
| `--comparison-operator <op>` | One of `>`, `>=`, `<`, `<=`, `==`, `!=` |
| `--condition-value <number>` | Threshold value to compare against |
| `--warmup-duration <minutes>` | Minutes the condition must hold before opening |
| `--cooldown-duration <minutes>` | Minutes the condition must clear before closing |
| `--notifier-ids <id,id>` | Comma-separated notifier IDs |
| `--tag key=value` | Trigger tags; repeat or comma-separate |
| `--description <text>` | Optional longer description or runbook hint |
| `--no-match-is-zero` | Treat missing datapoints as zero |
| `--dashboard-id <id>` | Link a dashboard in notifications |
| `--format <name>` | Value display format, such as `duration` or `percent` |
| `--format-input <name>` | Input unit for size formats |

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
| `--page-all` | Auto-paginate to fetch all results |

## Log Metric Options

Use `logs metrics` when the goal is to extract a metric from matching log lines so it can later be charted, alerted on, or queried elsewhere in AppSignal.

Useful `logs metrics create` and `logs metrics update` flags:

| Flag | Description |
|---|---|
| `--name <text>` | Required. Human-readable name for the metric configuration |
| `--query <text>` | Required. Log query used to match lines |
| `--source-id <id>` | Restrict matching to specific log sources; repeat the flag for multiple sources |
| `--metric 'name=...,type=...,field=...,tag.foo=bar'` | Required for `create`. Metric definition; repeat for multiple extracted metrics |
| `--clear-sources` | On update, remove all source restrictions |
| `--clear-metrics` | On update, remove all metric definitions |
| `--id <id>` | Required for `update` and `delete`. Existing metric configuration ID |

Metric definition notes:

- `type=counter` counts matching lines and does not require `field=...`
- `type=gauge` and `type=distribution` require `field=...`
- Use `tag.<name>=<value>` inside `--metric` to attach tags to the emitted metric

## Log Trigger Options

Use `logs triggers` when the goal is to get notified about matching log lines.

Useful `logs triggers create` and `logs triggers update` flags:

| Flag | Description |
|---|---|
| `--name <text>` | Required. Human-readable trigger name |
| `--query <text>` | Required. Log query used to match lines |
| `--source-id <id>` | Restrict matching to specific log sources; repeat for multiple sources |
| `--description <text>` | Optional longer description or runbook hint |
| `--notifier-id <id>` | Attach a notifier; repeat for multiple notifiers |
| `--severity <value>` | Match only specific severities; repeat for multiple values |
| `--clear-sources` | On update, remove all source restrictions |
| `--clear-notifiers` | On update, remove all notifiers |
| `--clear-severities` | On update, remove all severity filters |
| `--id <id>` | Required for `update` and `delete`. Existing trigger ID |

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
appsignal-cli auth login
```

Save the default org and list apps:

```bash
appsignal-cli apps list
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

Add an incident note. Note content supports Markdown; prefer Markdown for structured findings, actions, code, and links:

```bash
appsignal-cli incidents add-note --number 42 --app "MyApp" --environment "production" --content $'## Investigation\n\n- Root cause: connection pool exhaustion\n- Resolution: increased the pool limit'

# Update or delete a note using its AppSignal note ID
appsignal-cli incidents list-notes --number 42 --app "MyApp" --environment "production"
appsignal-cli incidents update-note --number 42 --app "MyApp" --environment "production" --id <note-id> --content "Investigation updated."
appsignal-cli incidents delete-note --number 42 --app "MyApp" --environment "production" --id <note-id>
```

Search logs with JSON output:

```bash
appsignal-cli --output json logs search --app "MyApp" --environment "production" --query "timeout"
```

List triggers for an app:

```bash
appsignal-cli triggers list --app "MyApp" --environment "production"
```

Create a trigger with distinct trigger and metric names:

```bash
appsignal-cli triggers create --app "MyApp" --environment "production" \
  --name "Slow web requests" \
  --metric-name response_time \
  --kind Advanced \
  --field mean \
  --comparison-operator ">" \
  --condition-value 500 \
  --description "Alert when mean response time stays above 500ms" \
  --warmup-duration 5 \
  --cooldown-duration 2
```

Search all matching logs in a time window:

```bash
appsignal-cli --output json logs search --app "MyApp" --environment "production" --start "2025-03-16T06:00:00Z" --query "group:notifiers" --page-all
```

Tail logs using a saved view:

```bash
appsignal-cli logs tail --app "MyApp" --environment "production" --view "Error logs"
```

Create a log-derived metric:

```bash
appsignal-cli logs metrics create --app "MyApp" --environment "production" \
  --name "Track error count" \
  --query 'severity:error' \
  --metric 'name=log.error_count,type=counter'
```

Update a log-derived metric and clear source restrictions:

```bash
appsignal-cli logs metrics update --app "MyApp" --environment "production" \
  --id metric_rule_123 \
  --clear-sources
```

Create a log-based trigger:

```bash
appsignal-cli logs triggers create --app "MyApp" --environment "production" \
  --name "Root login" \
  --query 'message:root' \
  --severity ERROR \
  --notifier-id notifier_123
```

Update a log-based trigger and remove all notifiers:

```bash
appsignal-cli logs triggers update --app "MyApp" --environment "production" \
  --id trigger_rule_123 \
  --clear-notifiers
```

Install the skill for Codex:

```bash
appsignal-cli skill install --target codex
```
