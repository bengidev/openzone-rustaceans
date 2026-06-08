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
  *panel-local* `subscription()` into one stream via `Subscription::batch`,
  plus a single workspace-level Clock tick gated on whether any Clock panel
  exists. Iced starts/stops each as panels appear and drop.
- The **app-root stores port**: `AppStores { counter: CounterStore, clock: ClockStore }`
  is *defined* here but *owned* at app root (`OpenZone` in `main.rs`). The
  workspace borrows `&mut AppStores` through its `update`; panels are views
  over store handles.

## Layout vocabulary

- **Pane** — a leaf of the center `pane_grid`. Splits happen *between* panes.
- **PaneState** — a pane's tab stack: `{ tabs: Vec<Box<dyn Panel>>, active }`.
  Tabs happen *within* a pane. Docks reuse this same type.
- **PanelLocation** — addresses any panel unambiguously: `Center(pane_grid::Pane)`
  or `Dock(DockSide)`.
- **Tab strip** — the clickable row of tab labels at the top of each pane;
  the active tab is highlighted via the `Accent` foreground token.

## The `Panel` port contract (final)

```text
title()                  -> String                       human-readable label
kind()                   -> PanelKind                    stable identity
view(&stores)            -> Element<ErasedMessage>       view-over-handle render
update(msg, &mut stores) -> ()                           fold intent; mutate self or store
subscription()           -> Subscription<ErasedMessage>  panel-local stream (default none)
snapshot(&stores)        -> serde_json::Value            handle-only persistence
on_close(&mut stores)    -> ()                           release any store slot (default none)
```

### View-over-handle

Counter and Clock panels do **not** own domain data. A `CounterPanel` carries
only a `CounterId`; a `ClockPanel` carries no per-instance state. Both read
their canonical value from `AppStores` on every render. `view`, `update`,
`snapshot`, and `on_close` all see the same store reference, so a frame is
internally consistent with no caching layer in the panel.

### Intent lifting

Panel messages are *intents* (`CounterMessage::Increment`, etc.) erased at the
trait boundary. The workspace reducer is the **single writer** of both layout
state (via `&mut self`) and domain state (via `&mut AppStores`):

1. A `WorkspaceMessage::Panel { location, tab, message }` arrives.
2. The reducer addresses the live panel and calls `panel.update(message, &mut stores)`.
3. The panel downcasts to its concrete intent and folds it — onto `self` for
   panel-local UI state, onto `stores` for domain state. There is **no
   interior mutability** anywhere; the whole path is `&mut`.

### Single store-level Clock subscription

The 1 Hz clock tick lives at the workspace layer, not on `ClockPanel`. The
workspace's `subscription` checks `has_clock_panel()` and, if any Clock panel
exists in the layout, batches in `iced::time::every(1s).map(|_| ClockTick)`.
Each `ClockTick` folds into one `ClockStore::tick()`; every Clock panel
re-renders against the same value (single-source fan-out). Removing the last
Clock tab also removes the gating reason for the subscription, so Iced stops
it without orphan streams.

### Message erasure

`dyn Panel` cannot carry an associated message type and stay object-safe, so
every panel **erases** its concrete message to `ErasedMessage`
(`Arc<dyn Any + Send + Sync>`).

- `Arc`, not `Box`: Iced widgets (`button`, `text_input`) require the
  application message to be `Clone`; `Box<dyn Any>` is not `Clone`.
- `Send + Sync` is mandatory — Iced `Task`s and `Subscription`s run on tokio.
- Each panel downcasts at its own boundary and `debug_assert!`s on a failed
  downcast: misrouting is loud in development, a silent no-op in release.

The workspace routes all panel messages through one path, so the panel that
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

Slices **1–4 of 6** are landed: single-window shell with three dummies (1),
edge docks + commands + panel-first key routing (2), handle-only layout
persistence with the `PanelRegistry` rehydrate path (3), and **app-root
stores + intent lifting + a single store-level Clock subscription** (this
slice, 4). Later slices add multi-window polish (5) and full custom tab
drag-and-drop (6). No real feature enters until the shell is proven with the
three dummies.
