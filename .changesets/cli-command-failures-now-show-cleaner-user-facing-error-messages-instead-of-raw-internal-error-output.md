---
bump: patch
type: change
---

CLI command failures now show cleaner user-facing error messages
instead of raw internal error output.

Unexpected internal failures now fall back to a generic safe
message by default, while `APPSIGNAL_CLI_DEBUG=1` still exposes the
underlying details for debugging.
