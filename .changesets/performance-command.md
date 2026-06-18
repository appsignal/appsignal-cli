---
bump: minor
type: add
---

Added a `performance` command for ranking slow work in an app. `performance actions` ranks the recent performance incidents by mean request duration (the default), total duration, or throughput, scoped with `--namespaces`, `--action`, and `--state`. `performance queries` takes the slowest actions, fetches the latest sample for each, and surfaces the slow database queries and N+1 suspects behind them — answering "why is this action slow?" without leaving the terminal. Both render as a table or, with `--output json`, as machine-readable JSON. Because the public API exposes performance data at the action level rather than per-SQL, the query view is derived from the latest sampled request for each action rather than a full aggregate, and the output states this.
