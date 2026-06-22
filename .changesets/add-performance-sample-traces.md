---
bump: patch
type: add
---

Added `traces` commands, also available as `samples`, for listing performance samples/traces and error traces, then inspecting their span trees through the AppSignal REST tracing API. Traces can also be fetched and inspected directly from performance and exception incident numbers, and `--page-all` can fetch beyond the first page of trace results.
