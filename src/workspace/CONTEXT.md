# CONTEXT: Workspace Shell

The **workspace shell** is the cross-cutting UI composition layer that future
assistant features (AI chat, sessions, editor, terminal) plug into. It is a
distinct **bounded context** — the *UI shell* — not a domain feature.

## What this context owns

- The **layout engine**: an outer frame of `column[title_bar, center, status_bar]`
  where the center is Iced's built-in `pane_grid` (recursive split tree between
  panes). Tab stacks live *within* each pane, separate from the split tree.
- The **`Panel` port**: the stable, final trait every panel implements.
- The **panel registry**: a `PanelKind -> constructor` table wired at the
  composition root. The composition seam for persistence rehydrate and (later)
  dynamic panel open / plugins.
- **Focus**: a single, centrally-owned `focused: PanelLocation`, kept in sync
  with the pane grid's own click focus. Chrome (active-pane border, active-tab
  highlight) is a pure read of this focus.
- **Subscription batching**: the workspace aggregates every live panel's
  `subscription()` into one stream via `Subscription::batch`; Iced starts/stops
  each as panels appear and drop.

## Layout vocabulary

- **Pane** — a leaf of the center `pane_grid`. Splits happen *between* panes.
- **PaneState** — a pane's tab stack: `{ tabs: Vec<Box<dyn Panel>>, active }`.
  Tabs happen *within* a pane. Docks (a later slice) reuse this same type.
- **PanelLocation** — addresses any panel unambiguously: `Center(pane_grid::Pane)`
  today; `Dock(DockSide)` is added later without changing existing call sites.
- **Tab strip** — the clickable row of tab labels at the top of each pane;
  the active tab is highlighted via the `Accent` foreground token.

## The `Panel` port contract (final)

```text
title()        -> String              human-readable tab / title label
kind()         -> PanelKind           stable identity for registry + persistence
view()         -> Element<ErasedMessage>   render; messages erased at the boundary
update(msg)    -> ()                  fold an erased message back into state
subscription() -> Subscription<ErasedMessage>   optional external stream (default none)
snapshot()     -> serde_json::Value   handle-only persistence (never full content)
```

### Message erasure

`dyn Panel` cannot carry an associated message type and stay object-safe, so
every panel **erases** its concrete message to `ErasedMessage`
(`Arc<dyn Any + Send + Sync>`).

- `Arc`, not `Box`: Iced widgets (`button`, `text_input`) require the
  application message to be `Clone`; `Box<dyn Any>` is not `Clone`.
- `Send + Sync` is mandatory — Iced `Task`s and `Subscription`s run on tokio.
- Each panel downcasts at its own boundary and `debug_assert!`s on a failed
  downcast: misrouting is loud in development, a silent no-op in release.

The workspace routes all panel messages through one path
(`WorkspaceMessage::Panel { location, tab, message }`), so the panel that
produced a message receives it back. A stale tag (panel since removed) is a
no-op, never a panic.

## Dependency rule

```text
features/<panel>  ──►  workspace   (to implement the Panel trait)
workspace         ──✗  a concrete feature   (never)
```

The shell addresses panels only through the `Panel` trait and `PanelKind`. It
never names a concrete feature type. `main.rs` is the one place that knows
concrete panels: it registers their constructors and builds the initial layout.

## Layout invariants

1. Exactly one `PanelLocation` is focused per window. `TabSelected` and
   `PaneClicked` both update it; chrome reads it.
2. Tab selection is range-checked — a stale index can never point `active` at a
   missing tab.
3. The center is the *only* resizable split region. The title bar and status
   bar are fixed chrome.
4. Subscription identity folds in the panel's location + tab so two panels of
   the same kind don't collapse into one stream.
5. Snapshots store **handles**, not content (a counter's value, a file path —
   never a file's bytes).

## Why the shell is not layered like a feature

Domain features follow `domain / application / infrastructure / presenter`. The
shell has **no business rules** — it is composition. Forcing the layered shape
would create contrived empty layers, so the shell is a flat top-level module.

## Build spine status

This slice is **step 1 of 6** (single-window shell): `pane_grid` + one
`PaneState`, tab strip, and Counter / Text / Clock as trivial `Panel` impls.
Later slices add docks + commands (2), registry persistence round-trip (3),
shared app-root stores + intent (4), multi-window (5), and full custom tab
drag-and-drop (6). No real feature enters until the shell is proven with the
three dummies.
