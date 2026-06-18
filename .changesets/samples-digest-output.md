---
bump: minor
type: add
---

`samples show` now prints an analysed digest of the sample by default instead of a flat field dump: a request overview with the acting user, a performance breakdown by event group, the slowest events and queries, N+1 query detection, and — for error samples — the exception, backtrace, error causes, and breadcrumbs. `--output json` returns this as a structured `analysis` object alongside the raw `sample`, and `--raw` prints the unprocessed sample instead of the digest.
