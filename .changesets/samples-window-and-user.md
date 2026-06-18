---
bump: minor
type: add
---

`samples list` can now scan a time window across incidents instead of requiring a single incident. Omit `--incident` and pass `--start`/`--end` to collect the samples recorded in that window across recent incidents — answering "what happened between T1 and T2" rather than "show me this one incident". `--namespaces` narrows which incidents are scanned and `--user` keeps only samples whose user identity (matched against the sample's overview, attributes, and session data) contains the given id, email, or substring. In window mode `--limit` caps how many recent incidents are scanned (default 20). `--user` also works in single-incident mode.
