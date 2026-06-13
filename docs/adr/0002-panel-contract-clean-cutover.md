# ADR 0002 — Panel Contract Clean Cutover

- **Status**: accepted
- **Date**: 2026-06-13
- **Decision makers**: workspace shell maintainers
- **Parents**: issue [#27](https://github.com/bengidev/openzone-rustaceans/issues/27) (workspace layout architecture cutover), [ADR 0001](0001-workspace-layout-state-model.md)

## Context

The current `Panel` trait was designed around the three early dummy panels
(Counter, Clock, Text) and the `AppStores` harness they exercise. As a result,
the panel contract carries several implementation assumptions that do not
generalise to real surfaces:

- **Mandatory snapshots**: every panel is expected to produce a persistence
  handle, even when a panel is inherently ephemeral or unsaved.
- **Owned-string titles and close copy**: returning `String` for `title()` and
  close-confirmation messages allocates even when the text is static, which
  matters in tab chrome that re-renders frequently.
- **No dirty state**: the shell cannot tell whether a panel is safe to close
  silently, so close-time behaviour for things like an unsaved text surface
  currently has to be bolted on outside the panel.
- **No close request**: close confirmation is currently an ad-hoc concern,
  not part of the panel's contract. The shell does not have a uniform way to
  ask panels "may I close you?".
- **App-root dummy stores in the contract**: the panel port still exposes
  app-root dummy stores to support demo panels, even though those panels will
  be deleted and real surfaces will own their persistence differently.
- **Production dummy panels**: Counter, Clock, and Text panels still ship in
  `features/dummies/` and are referenced from `main.rs` and the registry as
  the default panels of the shell.

Keeping the contract tightly coupled to these dummies makes it hard to add
real surfaces (Scratch, Conversation, Terminal) without either carrying dead
dummy machinery into production or building a parallel panel contract next to
it.

## Decision

Perform a clean cutover of the panel contract so that it reflects real
surfaces, not deleted demos.

### Remove production dummy panels

Delete the Counter, Clock, and Text dummy panels and any wiring that treats
them as default shipping content. Tests that require panel fixtures move to
test-only panels under `#[cfg(test)]` or an internal test support module;
they are never shipped as part of the production shell.

Dummy panels are removed end-to-end: types, constructors, registry entries,
startup layout references, persistence fixtures, and any documentation that
treats them as real surfaces. No deprecated aliases or compatibility shims are
left behind.

### Optional panel snapshots

Panels provide an optional snapshot handle instead of a mandatory one:

- Panels that are meaningful to persist (for example, a file editor panel)
  return a persistence handle.
- Panels that are ephemeral or unsaved (Scratch, passive output surfaces,
  one-off previews) return no snapshot and are omitted from layout
  persistence.

Persistence code treats "no snapshot" as "do not store, do not restore", and
composition-root fallbacks handle empty restores ([ADR 0001](0001-workspace-layout-state-model.md)).

### Dirty state

Panels expose a separate dirty-state signal distinct from close requests:

- A panel reports whether its current state is dirty (something the user
  would lose on close) independently of whether it wants to block close
  right now.
- Status chrome can read dirty state to render indicators such as a prefix
  dot on tabs, without invoking the close request flow.

### Close requests

Panels participate in close through a typed close request:

- When the workspace asks a panel whether it may be closed, the panel returns
  one of: allow immediately, or request user confirmation with a
  copy-on-write confirmation message.
- The shell owns the confirmation overlay and the aggregated window-close /
  app-quit flow; panels only provide the reason and message.
- Confirmation messages use copy-on-write (`Cow<'_, str>`) ownership so that
  static strings do not allocate every close check.

### Copy-on-write titles

Panel titles use copy-on-write ownership rather than owned `String`:

- Static titles (for example, `untitled`) are held as borrowed or static `Cow`
  values and do not allocate on tab re-renders.
- Dynamic titles (for example, a filename) remain allocated when needed.

The trait changes to return a `Cow<'_, str>` instead of `String`, and views
treat it as a read-only label.

### Status sink contributions

Panels contribute status segments via a shell-owned status sink rather than
allocating and returning a whole status list:

- The shell creates and owns a `StatusSink`.
- Panels push `Cow<'_, str>` segments into the sink when asked for their
  status contribution.
- Panels may cache dynamic labels internally (for example, cursor position) so
  unchanged status does not reallocate every frame.

The status bar reads the sink for the focused panel; panels never render the
bar itself.

### Remove dummy stores from the contract

App-root dummy stores (Counter, Clock stores) are removed from the panel
contract and the shell's `AppStores`-centric assumptions are removed. Real
surfaces either:

- own their own state and persistence, or
- consume app-root-provided stores through a typed composition seam,
  not a hard-coded dummy store surface.

The panel port continues to allow access to external state through a generic
port, but the contract is defined around the needs of real surfaces, not
deleted dummies.

### Generic panel subscriptions

Remaining Clock-like per-kind subscriptions in the shell are converted to
generic panel subscription batching. The shell no longer references any
concrete panel kind in subscription logic; it aggregates whatever subscriptions
live panels provide.

## Consequences

- `Panel` trait methods change: `title()` returns `Cow<'_, str>`, snapshots
  become optional, close requests and dirty state are added, and status
  contributions go through a sink.
- Counter, Clock, and Text types, plus any code path that references them
  from non-test code, are removed.
- `PanelKind` no longer includes Counter / Clock / Text variants; initial
  identity starts with Scratch only, and future real panels add kinds when
  they are implemented.
- The shell's startup, restore, and close code is rewritten to use the new
  optional snapshots, dirty state, close requests, and status sink instead of
  dummy-specific assumptions.
- Tests that previously depended on production dummy panels are re-implemented
  using test-only panels under `#[cfg(test)]`; these tests preserve the same
  reducer, persistence, and geometry seams.
- There are no deprecated aliases, aliases under a different name, or
  compatibility shims for the deleted dummy panels.
