# Architecture

OpenZone Rustaceans is a single Cargo package with internal modular boundaries.

## Composition root

`src/main.rs` is the application composition root. It chooses infrastructure
implementations, wires dependencies, registers panel constructors into the
shell registry, and launches the Iced runtime. It is a thin composition root:
no feature logic or shell behaviour accumulates there.

## Internal modules

```text
src/
├── main.rs                          # composition root
├── features/
│   ├── mod.rs
│   ├── dummies/                     # test/development dummy panels (removed in production builds)
│   ├── onboarding/
│   │   ├── mod.rs                   # feature facade
│   │   ├── onboarding_state.rs      # reducer, messages, dynamics
│   │   ├── onboarding_persistence.rs# persistence contracts / routing outcomes
│   │   ├── onboarding_messages.rs   # routing, side-effects
│   │   └── onboarding_view.rs       # Iced view rendering
│   └── scratch/                     # (planned) line-oriented unsaved text work surface
├── shared/
│   ├── mod.rs
│   └── design/
│       ├── mod.rs                   # facade
│       ├── design_tokens.rs         # palette, tokens
│       └── design_theme.rs          # theme resolver
└── workspace/
    ├── mod.rs                       # context facade
    ├── workspace_state.rs           # layout engine, reducer
    ├── workspace_message.rs         # workspace message types
    ├── workspace_view.rs            # workspace Iced view
    ├── workspace_stores.rs          # AppStores definition
    ├── workspace_drag.rs            # drag geometry and drop targets
    ├── workspace_panel.rs           # Panel port trait
    ├── workspace_command.rs         # command system, keymaps
    ├── workspace_registry.rs        # PanelKind -> constructor
    ├── workspace_pane_state.rs      # PaneState (tab stack)
    ├── workspace_dock.rs            # dock types and visibility
    ├── workspace_layout_metrics.rs  # layout spacing and metrics
    ├── workspace_location.rs        # PanelLocation addressing
    └── workspace_persistence.rs     # layout persistence
```

## Module conventions

### Feature-prefixed flat modules

Internal modules use **feature-driven flat files** with a context prefix. The
old `application/domain/infrastructure/presenter` folder pattern is replaced by
a single flat directory per context (feature, shared, or workspace).

Each implementation file carries the context prefix in its filename:

- `onboarding_state.rs`, `onboarding_view.rs`, `onboarding_persistence.rs`
- `workspace_state.rs`, `workspace_view.rs`, `workspace_message.rs`
- `design_tokens.rs`, `design_theme.rs`

### Conventional entrypoints

`main.rs` and `mod.rs` are **exempt from prefixing**. They remain conventional
entrypoints:

- `main.rs` — application composition root.
- `mod.rs` — per-module facade that re-exports the public surface.

### Import conventions

- **Inside a feature**, use sibling `super::` paths. Sibling modules
  import from each other directly.
- **Outside a feature**, callers use the feature's `mod.rs` facade.

## Boundary rules

- Keep `main.rs` thin: compose modules, register constructors, wire stores,
  and launch. No feature logic lives here.
- Keep domain contracts free from UI and filesystem details.
- Share cross-feature primitives through `shared`, not feature-to-feature
  imports.
- Use `pub(crate)` or private modules by default; expose only
  composition-facing APIs.

## Crate policy

Do not split a module into a standalone crate until there is a real external
consumer or publishable API. Internal modules should remain portable enough to
extract later, but are optimized for fast early-stage iteration.

## See also

- [ADR 0001 — Workspace Layout State Model](adr/0001-workspace-layout-state-model.md)
- [ADR 0002 — Panel Contract Clean Cutover](adr/0002-panel-contract-clean-cutover.md)
- [Workspace Glossary](../src/workspace/CONTEXT.md)
