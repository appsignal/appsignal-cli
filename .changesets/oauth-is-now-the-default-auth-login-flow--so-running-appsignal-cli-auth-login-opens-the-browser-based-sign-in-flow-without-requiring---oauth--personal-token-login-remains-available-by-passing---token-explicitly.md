---
bump: patch
type: change
---

OAuth is now the default auth login flow, so running appsignal-cli auth login opens the browser-based sign-in flow without requiring --oauth. Personal token login remains available by passing --token explicitly.
