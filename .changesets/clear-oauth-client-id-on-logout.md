---
bump: patch
type: fix
---

`auth logout` now removes the stored OAuth client ID along with the saved
credentials, so a later login no longer reuses a stale client ID from the
previous session.
