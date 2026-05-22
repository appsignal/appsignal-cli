---
bump: major
type: change
---

The CLI now blocks command execution when GitHub reports a newer major
`appsignal-cli` release and shows an upgrade-required message with the
current and latest versions.

Patch and minor releases still show a startup warning only, and GitHub
outages or other non-`200` responses still pass silently so the CLI is
not blocked by GitHub availability.
