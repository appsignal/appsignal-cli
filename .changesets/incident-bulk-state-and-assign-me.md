---
bump: minor
type: change
---

`incidents update` now accepts multiple incident numbers for bulk state changes, so you can close or reopen an explicit set of incidents in one command. It also adds `--assign-me` to assign the incident to the authenticated CLI user without needing to look up your user ID first.
