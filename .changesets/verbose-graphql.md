---
bump: minor
type: add
---

Added a global `--verbose` (`-v`) flag that prints each outgoing GraphQL request — its URL, query, and variables — to stderr before it is sent. This makes it easy to see exactly what the CLI asks the API for when debugging, and because the dump goes to stderr it never mixes with `--output json` on stdout.
