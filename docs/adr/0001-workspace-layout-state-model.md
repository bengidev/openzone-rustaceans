# ADR 0001 — Workspace Layout State Model

- **Status**: accepted
- **Date**: 2026-06-13
- **Decision makers**: workspace shell maintainers
- **Parents**: issue [#27](https://github.com/bengidev/openzone-rustaceans/issues/27) (workspace layout architecture cutover)

## Context

The current workspace shell uses a binary dock model: docks are either visible
or hidden, with no distinction between a collapsed rail-only affordance and a
fully hidden dock. The shell also implicitly assumes the Workbench always
contains at least one user-selected panel, which makes startup, empty restore,
and fallback scenarios awkward — the composition root has to special-case
placeholder insertion.

In addition, dock surfaces are named by the shell: the Workbench and each dock
know about concrete surface constructors, and opening an empty dock is
currently a concrete-feature concern. This leaks feature knowledge into shell
state transitions and makes the reducer harder to reason about.

Finally, dock extents (width/height) are not remembered per dock: when a dock
is hidden and then reopened, its size is not reliably restored. This is
acceptable for the shipped dummy panels but breaks the editor-like experience
targeted for real surfaces.

## Decision

Adopt a deterministic **2D Workbench-plus-docks** layout state model with the
following properties.

### 2D shell

The workspace remains a deterministic 2D shell. Introduce:

- no persistent freeform spatial canvas,
- no panel rotation,
- no persistent Z-depth.

Visual depth may still be used to signal hierarchy, elevation overlays, or
drag feedback, but panels do not live in a freeform spatial model.

### Workbench as the only split region

The Workbench is the center-only split region. Edge docks (Activity,
Conversation, Output) frame the Workbench and are not part of it. Only the
Workbench participates in recursive pane splits.

### Tri-state dock visibility

Every edge dock has one of three visibility states:

- **Hidden**: render neither rail nor body; consume no visible layout space.
- **Collapsed**: render the rail only; consume no body space.
- **Open**: render the body at the dock's remembered extent.

All three states retain the dock's tab stack. The current binary
visible/hidden model is replaced with this three-state model in shell state.

Transitions:

- Closing an Open dock -> Hidden.
- Collapsing an Open dock -> Collapsed.
- Opening a Hidden or Collapsed dock -> Open.
- Collapse and Hide are distinct actions and both are exposed in dock chrome.

Hidden docks are revealed from always-visible status-bar layout controls or
workspace commands, not from an invisible edge rail. Opening a Hidden or
Collapsed dock focuses that dock when it has interactive content. Passive
reveals and passive output badges never steal focus. Hiding the focused dock
returns focus to the last focused Workbench pane, or the first center pane if
none exists.

### Per-dock remembered extents

Each dock remembers its own visible extent:

- left and right docks remember their width,
- the Output Dock remembers its height.

Extents are persisted alongside dock visibility. On restore, an Open dock
returns at its remembered extent; a missing or zero extent falls back to the
dock's layout-default extent.

### Composition-root default surface factories

The workspace shell does not name any concrete surface type. Opening an empty
dock is resolved by a **composition-root factory**:

- Each dock side has an optional `FnMut() -> Box<dyn Panel>` factory supplied
  by `main.rs`.
- When the user explicitly opens an empty dock, the reducer emits an effect
  requesting the default surface for that dock side.
- The composition root invokes the factory, inserts the resulting panel, and
  re-emits the updated layout state to the reducer.
- On restore, an Open dock with no tabs invokes its factory if one exists;
  otherwise the dock restores Hidden.

This keeps the shell generic: the shell asks the composition root to produce a
panel, and never names the concrete surface type.

### Startup layout

The default first-open layout is:

- all edge docks Hidden,
- the Workbench filling the window,
- a single clean Scratch Pane focused with an `untitled` tab.

The composition root provides the clean Scratch fallback panel via a separate
Scratch factory. The shell does not construct or name Scratch.

New windows inherit the same startup layout and Hidden docks.

### Empty-Workbench guard

The Workbench must never be empty in any user-visible state. Whenever a
reducer transition would leave the Workbench with no panes or no active tabs,
the reducer emits an effect requesting a Scratch fallback from the
composition root. The root inserts a clean Scratch Pane and re-enters the
reducer, so the observable state never includes an empty Workbench.

## Consequences

- The reducer transitions from a binary to a tri-state dock visibility model,
  which is a breaking change to workspace state and any persistence that
  records dock visibility.
- Shell persistence now stores per-dock visibility, per-dock extents, and
  optional active tab. Dock bodies and panel contents are still not stored.
- The composition root grows factories for Activity, Conversation, Output, and
  Scratch fallback. Existing dummy wiring is removed in the panel contract
  cutover ([ADR 0002](0002-panel-contract-clean-cutover.md)).
- View code reads the new tri-state visibility and renders rail/body
  accordingly; hit-testing and drag geometry must treat Hidden docks as
  outside the visible layout except during active drags.
