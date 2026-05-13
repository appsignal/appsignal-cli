---
bump: patch
type: fix
---

OAuth login now completes through a local browser callback, so signing in works on Linux without custom URI scheme support. Custom AppSignal endpoints must be configured as base URLs like `https://staging.lol`, and non-production environments can use a separate OAuth client ID when needed.
