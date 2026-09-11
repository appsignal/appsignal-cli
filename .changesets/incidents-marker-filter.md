---
bump: minor
type: add
---

`incidents list` now accepts `--marker <id>` to scope incidents to a single
deploy marker, which makes "what broke in this deploy?" a one-command lookup.
The marker id comes from `apps resources deploy-markers`. The filter maps to the
GraphQL `incidents(marker:)` argument and is opt-in — omitting `--marker` leaves
listing unchanged.
