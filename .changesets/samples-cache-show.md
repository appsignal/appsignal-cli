---
bump: minor
type: add
---

Added `samples cache show <sample-id>`, which re-renders the full investigator digest of a previously cached sample entirely offline — no API call. The local cache already stores complete samples, so this surfaces the same analysed digest (or, with `--raw`, the unprocessed sample) that `samples show` produces, even when the incident is closed or you have no network access. Scope the lookup to one application with `--app-id`, and use `--output json` for the digest plus the cached sample and the time it was cached.
