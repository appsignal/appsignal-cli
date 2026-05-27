---
bump: patch
type: change
---

OAuth login now supports custom AppSignal endpoints that advertise RFC 7591
Dynamic Client Registration.

When a custom endpoint exposes OAuth authorization server metadata with a
`registration_endpoint`, the CLI automatically registers a public client for its
loopback callback URL and uses the returned `client_id` for login and token
refresh. Endpoints without dynamic registration support still fall back to the
built-in production client ID unless an explicit `oauth_client_id` override is
configured.
