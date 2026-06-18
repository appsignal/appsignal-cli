---
bump: minor
type: change
---

The `samples show` performance digest now warns when the API truncated the
sample's timeline. AppSignal drops timeline events for very large samples, which
silently understated the performance breakdown and slowest-events lists. When
this happens the digest now prints `⚠ N timeline event(s) truncated by the API
— the breakdown and slowest-events below are understated.`, and `--output json`
includes a `truncated_events` count on the performance analysis. Samples whose
timeline was not truncated are unaffected.
