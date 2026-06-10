---
bump: patch
type: change
---

appsignal-cli now sends a minimal best-effort telemetry event for each command run to help measure CLI usage and reliability. The event includes only the command path, success or failure, duration, CLI version, and output format, and you can disable it entirely with `APPSIGNAL_CLI_TELEMETRY=0`.
