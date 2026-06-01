# Domain Docs

This repo can use a **multi-context** layout: a `CONTEXT-MAP.md` at the root points at one `CONTEXT.md` per context.

## Before exploring, read these

- **`CONTEXT-MAP.md`** at the repo root, if present.
- Per-context **`CONTEXT.md`** files referenced by the map.
- **`docs/adr/`**, if present, for accepted architectural decisions.
- Context-scoped ADRs such as `src/<context>/docs/adr/`, if present.

If any of these files do not exist, proceed silently. Do not flag their absence or suggest creating them upfront.

## Current domain language

- **Desktop Assistant** — the user-facing app that helps with work on the machine.
- **Desktop Context** — local information intentionally made available to the assistant.
- **AI Provider** — a cloud or local model backend used to process requests.
- **Assistant Workflow** — an end-to-end task the assistant can help plan, automate, or complete.
- **User Control Boundary** — the consent and permission boundary around local context, desktop actions, and model calls.

## Use the glossary's vocabulary

When output names a domain concept in an issue, refactor proposal, test, or documentation, use terms defined here or in relevant `CONTEXT.md` files. Avoid drifting to synonyms when a canonical term exists.

## Flag ADR conflicts

If a proposed change contradicts an accepted ADR, stop and flag it rather than silently overriding the decision.
