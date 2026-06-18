---
bump: patch
type: change
---

API requests now automatically retry when the server provably did not process
them: HTTP 429 responses (honoring the `Retry-After` header) and connection
establishment errors, with capped exponential backoff (up to 3 retries). This
makes the CLI resilient to transient rate limiting — common during
`logs search --page-all` or rapid scripted runs — without changing any output.
5xx responses and timeouts are deliberately not retried, because GraphQL
requests also carry mutations (incident updates, notes, triggers) and retrying
an ambiguous failure could double-apply one the server already processed.
