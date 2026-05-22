---
bump: minor
type: change
---

`appsignal-cli` now supports anomaly detection trigger management with `triggers list`, `triggers create`, `triggers update`, and `triggers archive`.

This adds CLI workflows for inspecting existing triggers, creating new metric alerts, updating trigger definitions as new versions, and archiving triggers when they are no longer needed.

The trigger output and shared skill docs also now distinguish clearly between the trigger name, metric name, and description so trigger configuration is easier to understand.
