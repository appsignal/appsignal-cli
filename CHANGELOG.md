# AppSignal CLI changelog

## 2.1.0

_Published on 2026-08-03._

### Added

- Added `--notification-frequency` and `--notification-threshold` to `incidents update`, so incident notification behavior, including nth-occurrence notifications, can be configured from the CLI. (minor [39cbb2a](https://github.com/appsignal/appsignal-cli/commit/39cbb2a6208923c6d2d5ad220c009829f6da0f9a))
- Trace listing commands now accept `--query`, allowing performance and error samples to be filtered by transaction tags such as `tag.region=eu-west` or by revision. (patch [39cbb2a](https://github.com/appsignal/appsignal-cli/commit/39cbb2a6208923c6d2d5ad220c009829f6da0f9a))
- Added incident note listing, update, and delete commands, so you can find note IDs and edit or remove notes you authored. (patch [39cbb2a](https://github.com/appsignal/appsignal-cli/commit/39cbb2a6208923c6d2d5ad220c009829f6da0f9a))

### Changed

- The bundled AppSignal skill now documents and recommends Markdown for structured incident notes, including findings, actions, code, and links. (patch [39cbb2a](https://github.com/appsignal/appsignal-cli/commit/39cbb2a6208923c6d2d5ad220c009829f6da0f9a))

## 2.0.2

_Published on 2026-07-20._

### Added

- Added appsignal-cli feedback so users and LLM workflows can send missing endpoint, feature, or broken behavior reports directly from the CLI, with optional contact email reuse for follow-up. (patch [ce8f6d0](https://github.com/appsignal/appsignal-cli/commit/ce8f6d0b5f9c1b3e475e18334c821d69f43aaaba))

### Changed

- Exception incident details now show error causes when sample data is available, making wrapped root causes visible directly from incidents show. (patch [ce8f6d0](https://github.com/appsignal/appsignal-cli/commit/ce8f6d0b5f9c1b3e475e18334c821d69f43aaaba))

## 2.0.1

_Published on 2026-07-01._

### Added

- Added `traces` commands, also available as `samples`, for listing performance samples/traces and error traces, then inspecting their span trees through the AppSignal REST tracing API. Traces can also be fetched and inspected directly from performance and exception incident numbers, and `--page-all` can fetch beyond the first page of trace results. (patch [7917561](https://github.com/appsignal/appsignal-cli/commit/7917561386b6a5aaed4d4c356e11b856f38cf6a1))

### Changed

- Incident list commands now render with the same table formatting as other list commands, making human-readable output easier to scan. (patch [7917561](https://github.com/appsignal/appsignal-cli/commit/7917561386b6a5aaed4d4c356e11b856f38cf6a1))

## 2.0.0

_Published on 2026-06-17._

### Changed

- `apps list` now uses the current OAuth account to determine the organization automatically and saves that org to the active config. You no longer need to pass `--org` when listing apps for the signed-in account. (minor [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))
- `incidents update` now accepts multiple incident numbers for bulk state changes, so you can close or reopen an explicit set of incidents in one command. It also adds `--assign-me` to assign the incident to the authenticated CLI user without needing to look up your user ID first. (minor [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))
- OAuth login now requests the `user:read` scope needed for browser-based AppSignal account access, and older tokens now produce a clearer re-authentication message when that scope is missing. The unsupported `apps orgs` command has been removed. (minor [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))
- Added dashboard management commands to list, create, and update AppSignal dashboards from the CLI. (patch [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))
- appsignal-cli now sends a minimal best-effort telemetry event for each command run to help measure CLI usage and reliability. The event includes only the command path, success or failure, duration, CLI version, and output format, and you can disable it entirely with `APPSIGNAL_CLI_TELEMETRY=0`. (patch [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))
- OAuth is now the default auth login flow, so running appsignal-cli auth login opens the browser-based sign-in flow without requiring --oauth. Personal token login remains available by passing --token explicitly. (patch [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))

### Removed

- Authentication is now OAuth-only. `appsignal-cli auth login` opens the browser flow directly, and personal API token login is no longer supported. (major [da7b53e](https://github.com/appsignal/appsignal-cli/commit/da7b53e9ee386c3463dda26079219569bd22883b))

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
