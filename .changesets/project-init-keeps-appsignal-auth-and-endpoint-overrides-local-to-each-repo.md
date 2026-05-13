---
bump: patch
type: fix
---

Projects can now keep AppSignal auth, endpoint, OAuth client ID, and default org settings in a local `.appsignal.toml` created with `appsignal-cli project init`. When that file exists, the CLI uses it as the only config for that project, so missing values no longer fall back to global settings, and project logout clears only the active project's credentials.
