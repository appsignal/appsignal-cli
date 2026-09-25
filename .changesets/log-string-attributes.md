---
bump: patch
type: fix
---

Log searches and tails now handle JSON-encoded attribute strings without failing the entire result. Unstructured attribute strings are preserved in a raw field.
