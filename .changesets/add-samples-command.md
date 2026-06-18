---
bump: minor
type: add
---

Added a `samples` command to fetch the underlying transaction samples behind an incident, which `incidents show` does not expose. `samples show` fetches a single sample — the latest, one by id (`--sample-id`), or the one closest to a timestamp (`--at`) — and `samples list` returns an incident's samples, optionally narrowed with `--start`/`--end`/`--limit`. Both accept an AppSignal incident or sample URL (or a bare sample id) as a positional argument, or the explicit `--incident` plus the usual `--app-id`/`--app`/`--environment`/`--org` flags, and both support `--output json`.
