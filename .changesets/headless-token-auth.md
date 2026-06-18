---
bump: minor
type: add
---

Added headless authentication with a personal API token, for CI and other non-interactive environments where the OAuth browser flow is impractical. Provide a token with the global `--api-token` flag or the `APPSIGNAL_API_TOKEN` environment variable and the CLI sends it as a `?token=` query parameter instead of running OAuth. A token supplied this way takes precedence over any stored OAuth credentials and is never written to disk; OAuth remains the default when no token is given. `appsignal-cli auth status` (and `about`) now report which authentication method is active.
