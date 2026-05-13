# AppSignal CLI changelog

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
