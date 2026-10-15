---
bump: patch
type: change
---

Stop sending and requesting the `severities` field for log views and log triggers. Severity filters are now part of the log query. `logs triggers create --severity` and `logs triggers update --severity` add the severities to the query as `severity=[...]`, and `logs triggers update --severity` now requires `--query`. The `logs triggers update --clear-severities` flag has been removed; edit the query instead.
