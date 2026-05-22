---
bump: patch
type: change
---

The CLI now checks the latest GitHub tag on startup and shows a boxed
warning when a newer `appsignal-cli` release is available.

The warning is only shown when GitHub returns a successful `200`
response with a newer version, so GitHub outages and other request
failures do not interrupt normal CLI usage.
