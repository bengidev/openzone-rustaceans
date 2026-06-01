# Roadmap

OpenZone Rustaceans is in early development. This roadmap tracks the intended direction for the desktop AI assistant.

## Phase 1 — Rust foundation

- Define application core boundaries
- Add configuration loading
- Add structured error handling and logging
- Establish CI checks for format, lint, and tests

## Phase 2 — AI provider layer

- Define provider trait/interface
- Add request/response types for model calls
- Support secure API key configuration
- Add provider capability metadata

## Phase 3 — Desktop context and permissions

- Define desktop context model
- Add explicit consent flows for context sharing
- Add redaction and sensitive-data filtering
- Document desktop permission boundaries

## Phase 4 — Assistant workflows

- Add first workflow execution path
- Add tool/action registry
- Add audit trail for assistant actions
- Add user review before sensitive actions

## Phase 5 — Desktop runtime

- Choose native desktop runtime/UI layer
- Implement first desktop shell
- Package development and release builds
- Document supported platforms
