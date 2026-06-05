# AppSignal CLI changelog

## 1.0.1

_Published on 2026-06-05._

### Changed

- The bundled AppSignal skill now shows how to turn common AppSignal URLs into CLI arguments, including `--org`, `--app-id`, `--number`, `--view`, and `--source-ids`. (patch [e86d5c8](https://github.com/appsignal/appsignal-cli/commit/e86d5c87ac08c34f39a091b7384a33bbe58ca13c))
- GraphQL API errors for account restrictions now show AppSignal's descriptive message, such as locked-account or free-plan quota explanations, instead of a generic request rejection. (patch [e86d5c8](https://github.com/appsignal/appsignal-cli/commit/e86d5c87ac08c34f39a091b7384a33bbe58ca13c))

### Fixed

- Fix log search order casing. Log searches now send the requested sort order in the format the AppSignal logs API expects, so commands like `appsignal-cli logs search --order desc` no longer fail with an unprocessable pagination order error. (patch [e86d5c8](https://github.com/appsignal/appsignal-cli/commit/e86d5c87ac08c34f39a091b7384a33bbe58ca13c))

## 1.0.0

_Published on 2026-05-27._

### Added

- Added `skill update` to refresh installed AppSignal LLM skills and `skill status` to list OpenCode, Codex, and Claude installs as current, outdated, missing, or unversioned. (minor [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))

### Changed

- The CLI now blocks command execution when GitHub reports a newer major
  `appsignal-cli` release and shows an upgrade-required message with the
  current and latest versions.

  Patch and minor releases still show a startup warning only, and GitHub
  outages or other non-`200` responses still pass silently so the CLI is
  not blocked by GitHub availability.

  (major [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))
- `appsignal-cli` now supports anomaly detection trigger management with `triggers list`, `triggers create`, `triggers update`, and `triggers archive`.

  This adds CLI workflows for inspecting existing triggers, creating new metric alerts, updating trigger definitions as new versions, and archiving triggers when they are no longer needed.

  The trigger output and shared skill docs also now distinguish clearly between the trigger name, metric name, and description so trigger configuration is easier to understand.

  (minor [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))
- Added --format as a synonym for the global --output flag, so you can use either name when selecting human or JSON CLI output. (patch [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))
- CLI command failures now show cleaner user-facing error messages
  instead of raw internal error output.

  Unexpected internal failures now fall back to a generic safe
  message by default, while `APPSIGNAL_CLI_DEBUG=1` still exposes the
  underlying details for debugging.

  (patch [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))
- OAuth login now supports custom AppSignal endpoints that advertise RFC 7591
  Dynamic Client Registration.

  When a custom endpoint exposes OAuth authorization server metadata with a
  `registration_endpoint`, the CLI automatically registers a public client for its
  loopback callback URL and uses the returned `client_id` for login and token
  refresh. Endpoints without dynamic registration support still fall back to the
  built-in production client ID unless an explicit `oauth_client_id` override is
  configured.

  (patch [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))
- The CLI now checks the latest GitHub tag on startup and shows a boxed
  warning when a newer `appsignal-cli` release is available.

  The warning is only shown when GitHub returns a successful `200`
  response with a newer version, so GitHub outages and other request
  failures do not interrupt normal CLI usage.

  (patch [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))

### Fixed

- `auth logout` now removes the stored OAuth client ID along with the saved
  credentials, so a later login no longer reuses a stale client ID from the
  previous session.

  (patch [a054e9b](https://github.com/appsignal/appsignal-cli/commit/a054e9bc4207e3ed391a5ddfeb7ba6358971ba35))

## 0.2.1

_Published on 2026-05-21._

### Changed

- Internal changes. (patch [7835bba](https://github.com/appsignal/appsignal-cli/commit/7835bba0e3303183ba58df48063b86c355b7a31a))

## 0.2.0

_Published on 2026-05-13._

### Added

- Added `appsignal-cli about`, a splash-style overview command that shows your CLI version, configured endpoint, authentication status, and suggested next commands. (minor [272fa73](https://github.com/appsignal/appsignal-cli/commit/272fa73b601a54531d25e1fc3eab6edd6414fd5b))
- Added dedicated `apps resources` subcommands such as `users`, `dashboards`, and `deploy-markers`, plus `apps resources all` for the combined view. `apps resources deploy-markers` now lists recent deploy markers, and `apps resources namespaces` now works with the current AppSignal GraphQL namespace shape. (minor [272fa73](https://github.com/appsignal/appsignal-cli/commit/272fa73b601a54531d25e1fc3eab6edd6414fd5b))
- Added `skill install` so you can install the bundled AppSignal CLI skill for OpenCode, Codex, or Claude. The installed skill explains the user-facing CLI commands and LLM-friendly output like `--output json`. (minor [272fa73](https://github.com/appsignal/appsignal-cli/commit/272fa73b601a54531d25e1fc3eab6edd6414fd5b))

### Fixed

- OAuth login now completes through a local browser callback, so signing in works on Linux without custom URI scheme support. Custom AppSignal endpoints must be configured as base URLs like `https://staging.lol`, and non-production environments can use a separate OAuth client ID when needed. (patch [272fa73](https://github.com/appsignal/appsignal-cli/commit/272fa73b601a54531d25e1fc3eab6edd6414fd5b))
- Projects can now keep AppSignal auth, endpoint, OAuth client ID, and default org settings in a local `.appsignal.toml` created with `appsignal-cli project init`. When that file exists, the CLI uses it as the only config for that project, so missing values no longer fall back to global settings, and project logout clears only the active project's credentials. (patch [272fa73](https://github.com/appsignal/appsignal-cli/commit/272fa73b601a54531d25e1fc3eab6edd6414fd5b))

## 0.1.0

_Published on 2026-03-26._

### Added

- Initial test release. (minor [ce1ab6d](https://github.com/appsignal/appsignal-cli/commit/ce1ab6db071c3e927208b2b192ebb2b8efd1d705))
