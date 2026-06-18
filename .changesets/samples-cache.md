---
bump: minor
type: add
---

Fetched transaction samples are now cached locally so they can be re-inspected offline. `samples show` and `samples list` write each sample they fetch to a per-sample JSON file under the platform cache directory (best-effort — caching never blocks or fails a fetch, and it does not change their output). A new `samples cache` command works with that cache without touching the API: `samples cache list` shows recently cached samples newest-first (filterable by `--app-id`), `samples cache search <query>` finds cached samples by their contents (action, user, query bodies, exceptions, and more), `samples cache clear` deletes them all, and `samples cache path` prints the cache directory. Because samples can contain request parameters and session data, caching can be disabled per call with `--no-cache` or globally with the `APPSIGNAL_NO_CACHE` environment variable.
