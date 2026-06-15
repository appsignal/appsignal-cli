---
bump: minor
type: change
---

OAuth login now requests the `user:read` scope needed for browser-based AppSignal account access, and older tokens now produce a clearer re-authentication message when that scope is missing. The unsupported `apps orgs` command has been removed; use `apps list --org <org-slug>` to select and save the organization you want to work with.
