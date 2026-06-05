---
bump: patch
type: fix
---

Fix log search order casing. Log searches now send the requested sort order in the format the AppSignal logs API expects, so commands like `appsignal-cli logs search --order desc` no longer fail with an unprocessable pagination order error.
