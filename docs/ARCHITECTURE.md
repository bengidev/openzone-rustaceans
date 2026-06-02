# Architecture

OpenZone Rustaceans is currently a single Cargo package with internal modular boundaries.

## Composition root

`src/main.rs` is the application composition root. It chooses infrastructure implementations, wires dependencies, and launches the Iced app.

## Internal modules

```text
src/
├── main.rs
├── features/
│   ├── mod.rs
│   └── onboarding/
│       ├── application/      # state reducer, messages, dynamics
│       ├── domain/           # persistence contracts and routing outcomes
│       ├── infrastructure/   # filesystem and memory persistence
│       └── presenter/        # Iced view and canvas rendering
└── shared/
    └── design/               # palette, tokens, theme resolver
```

`features::onboarding` is an internal vertical feature module, not a publishable crate. It keeps clean boundaries through domain traits and a small façade exposed from `src/features/onboarding/mod.rs`.

`shared` contains reusable internal building blocks. `shared::design` replaces the former design crate as an internal design-system module.

## Boundary rules

- Keep `main.rs` thin: compose modules, do not hold feature logic.
- Keep domain contracts in `features/onboarding/domain` free from UI and filesystem details.
- Put concrete adapters in `features/onboarding/infrastructure`.
- Put Iced view/rendering code in `features/onboarding/presenter`.
- Share cross-feature primitives through `shared`, not feature-to-feature imports.
- Use `pub(crate)` or private modules by default; expose only composition-facing APIs.

## Crate policy

Do not split a module into a standalone crate until there is a real external consumer or publishable API. Internal modules should remain portable enough to extract later, but are optimized for fast early-stage iteration.
