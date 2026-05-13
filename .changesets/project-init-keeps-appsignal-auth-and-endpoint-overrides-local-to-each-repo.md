---
bump: patch
type: fix
---

Projects can now keep AppSignal auth, endpoint, OAuth client ID, and default org settings in a local `.appsignal.toml` created with `appsignal-cli project init`. That makes it easier to use different AppSignal environments across repos without extra per-command flags, and project logout now clears only the active project's credentials instead of falling back to global auth.
