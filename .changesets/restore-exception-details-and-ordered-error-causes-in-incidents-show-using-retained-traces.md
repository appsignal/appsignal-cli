---
bump: patch
type: fix
---

Restore exception details and ordered error causes in incidents show using retained traces. Cause backtrace locations remain available. If optional details cannot be loaded within ten seconds, the command preserves available results and warns on stderr without failing.
