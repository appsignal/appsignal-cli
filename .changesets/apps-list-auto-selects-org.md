---
bump: minor
type: change
---

`apps list` now uses the current OAuth account to determine the organization automatically and saves that org to the active config. You no longer need to pass `--org` when listing apps for the signed-in account.
