---
bump: minor
type: add
---

Added a `metrics` command for working with an app's metrics directly from the terminal, served by the public GraphQL API. `metrics list` discovers the metric keys an app reports (name, type, fields, and tags), optionally filtered by `--name` fragment and capped with `--limit`. `metrics timeseries --metric <name>` fetches a metric's values over time, narrowed by `--field` and `--tag key=value` and windowed with either `--timeframe` (e.g. `R1H`) or both `--start` and `--end`. `metrics history --start <ISO> --end <ISO>` reports per-action error and performance throughput over an arbitrary window, scoped with `--namespaces`. All subcommands accept the usual `--app-id`/`--app`/`--environment`/`--org` flags and render as a table or, with `--output json`, as machine-readable JSON.
