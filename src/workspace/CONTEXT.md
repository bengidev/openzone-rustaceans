# CONTEXT: Workspace Shell

The **workspace shell** is the cross-cutting UI composition layer that future
assistant features (AI chat, sessions, editor, terminal) plug into. It is a
distinct **bounded context** — the *UI shell* — not a domain feature.

The shell does not name any concrete feature type. It addresses panels only
through the `Panel` trait and `PanelKind`. The composition root (`main.rs`)
is the one place that knows concrete panels: it registers their constructors,
wires their stores, and builds the initial layout.

## Glossary

- **Workspace layout model** — a deterministic 2D shell. No persistent freeform
  spatial canvas, panel rotation, or Z-depth. Visual depth may signal
  hierarchy, elevation, or drag feedback, but panels do not have freeform
  position or rotation.

- **Workbench** — the center-only work area where the user performs the
  primary task. The Workbench is the only split region. Open docks frame the
  Workbench but are not part of it. The Workbench must never be empty in
  user-visible state.

- **Activity Dock** — the left edge dock for tabbed navigation and context
  surfaces, such as project navigation, session lists, or workspace-level
  activity views.

- **Conversation Dock** — the right edge dock for assistant conversation and
  prompt entry. Conversation Surfaces default here but can be moved like any
  other panel.

- **Output Dock** — the bottom edge dock for terminal, logs, diagnostics, or
  other transient output. User-invoked output opens the dock; passive output
  badges it without stealing focus.

- **Dock visibility** — a tri-state: Hidden, Collapsed, or Open. Hidden docks
  render neither rail nor body and consume no layout space. Collapsed docks
  render a visible rail only. Open docks render their body. All three states
  retain the dock's tab stack. Hidden docks are revealed from status-bar
  layout controls or workspace commands. Closing an open dock hides it.
  Collapsing an open dock is a distinct action that leaves the rail visible
  for quick access. Opening a hidden or collapsed dock shows its body.
  Explicitly opening a dock focuses it when it has interactive content.
  Hiding the focused dock returns focus to the last focused Workbench pane.

- **Dock extent** — the remembered visible width of a side dock or height of
  the Output Dock. Each dock keeps its own extent so reopening it restores the
  user's last chosen size. Extents are persisted with layout state.

- **Startup layout** — the default first-open layout: all edge docks are
  hidden, and the focused Workbench fills the window with one clean Scratch
  Pane.

- **Pane** — a leaf of the Workbench split tree. Splits happen between panes.

- **PaneState** — a pane's tab stack: an ordered list of tabs and the index of
  the active tab. Docks reuse this same type.

- **PanelLocation** — addresses any panel unambiguously: either a Workbench
  pane or a named edge dock.

- **Tab strip** — the clickable row of tab labels at the top of each pane or
  dock body. The active tab is highlighted via accent foreground or underline.

- **Status Contribution** — status-bar segments supplied by the focused panel,
  such as cursor position, language mode, diagnostics, or task state. The
  shell owns the status bar layout; panels push segments into a shell-owned
  status sink and do not render the bar. Panels may cache dynamic labels so
  unchanged status does not allocate every frame.

- **Default Dock Surface** — the first surface created for an empty dock when
  the user explicitly opens it. Default dock surfaces are lazy-created by
  per-dock factories supplied from the composition root; the workspace shell
  does not name concrete surface types.

- **Command Center** — the persistent command/search trigger in the title bar,
  labelled `Search commands`. It is workspace chrome, not a panel or overlay.
  Activating it opens a transient command palette overlay scoped to the
  workspace. The workspace shell owns the palette's open/query UI state;
  command providers are supplied by the composition root. The palette shortcut
  is Cmd/Ctrl+Shift+P.

- **Persistence Handle** — an optional panel-provided handle used to rehydrate
  durable layout entries. Panels without a persistence handle are omitted from
  layout snapshots; the composition root restores required fallback surfaces
  when the Workbench would otherwise be empty.

- **Scratch Pane** — an editable, unsaved, non-domain plain-text work surface
  shown in the Workbench when no file, session, or project-specific panel has
  been chosen. Its first implementation is a line-oriented input, not a full
  text editor. It is ephemeral until saved or promoted by a feature; otherwise
  it is a fallback for first-open or invalid/empty Workbench restore, not
  durable content. Its user-facing tab label is `untitled`. Closing a dirty
  Scratch Pane warns rather than silently persisting or discarding its text.
  The composition root provides this fallback panel; the workspace shell does
  not name or construct a concrete Scratch Pane type.

- **Close Request** — a panel's response when the workspace asks whether it
  may be closed. Clean panels allow close immediately; dirty panels request
  user confirmation before the workspace removes them. The workspace shell
  owns the confirmation overlay; panels only provide the close reason and a
  copy-on-write confirmation message.

- **Conversation Surface** — one assistant conversation hosted as a tab whose
  default home is the Conversation Dock. The dock is only the default
  container; the surface may move to the Workbench or another dock like any
  other panel.
