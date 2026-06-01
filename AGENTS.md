# AGENTS.md

Guidance for AI coding agents working in this repo.

## Agent skills

### Issue tracker

Issues + PRDs are tracked as GitHub Issues (`gh` CLI), repo `bengidev/openzone-rustaceans`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary; each label string equals its canonical role name. See `docs/agents/triage-labels.md`.

### Domain docs

Multi-context layout guidance lives in `docs/agents/domain.md`. If `CONTEXT-MAP.md`, per-context `CONTEXT.md`, or ADR files exist, read the relevant ones before changing domain language or architecture.

## Project context

OpenZone Rustaceans is an early Rust foundation for a desktop AI assistant. Treat privacy, desktop permissions, and AI provider boundaries as first-class design constraints.
