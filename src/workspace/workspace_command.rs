#![allow(dead_code)]

//! Workspace commands and the keymap that drives them.
//!
//! Key routing is panel-first: the focused panel consumes a key chord if
//! it wants it (e.g. a text input swallowing a character); otherwise the
//! chord bubbles to the workspace [`Keymap`], which resolves it to a
//! [`Command`]. Commands are the stable, serializable vocabulary of
//! workspace-level actions — keybindings, menu items, and (later)
//! command-palette entries all resolve to the same enum.

use crate::workspace::workspace_location::DockSide;

/// A workspace-level action.
///
/// Commands are intentionally coarse and declarative: they name *what*
/// should happen, not *how*. The reducer ([`Workspace::apply_command`])
/// owns the how. Keeping this enum free of widget/runtime types lets the
/// keymap, future menus, and the command palette share one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// Open a dock (Hidden→Open, Collapsed→Open, Open→no-op).
    OpenDock(DockSide),
    /// Collapse an open dock (Open→Collapsed, others no-op).
    CollapseDock(DockSide),
    /// Hide a dock (Open→Hidden, Collapsed→Hidden, Hidden→no-op).
    HideDock(DockSide),
    /// Split the focused center pane, creating a sibling pane. Docks
    /// cannot be split, so this is a no-op when a dock is focused.
    SplitFocused,
    /// Close the active tab in the focused panel's pane. Closing the
    /// last tab collapses the pane (center) or the dock (edge).
    CloseActiveTab,
}

/// A keyboard modifier set, reduced to what the workspace keymap cares
/// about. Mirrors the bits of [`iced::keyboard::Modifiers`] we bind on
/// without leaking the runtime type into the keymap vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        command: false,
        shift: false,
        alt: false,
    };

    /// Primary accelerator (Cmd on macOS, Ctrl elsewhere — Iced folds
    /// both into `command`).
    pub const CMD: Mods = Mods {
        command: true,
        shift: false,
        alt: false,
    };

    pub const CMD_SHIFT: Mods = Mods {
        command: true,
        shift: true,
        alt: false,
    };

    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }
}

/// A keyboard key, reduced to the subset the keymap binds. Decoupled
/// from [`iced::keyboard::Key`] so reducer tests construct chords
/// without a runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyRef {
    Char(char),
    Backspace,
    Escape,
    Enter,
    ArrowUp,
    ArrowDown,
}

/// A fully-qualified key chord: a key plus its modifier set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: KeyRef,
    pub mods: Mods,
}

impl Chord {
    pub fn new(key: KeyRef, mods: Mods) -> Self {
        Self { key, mods }
    }

    /// A character chord with the given modifiers. Characters are
    /// lowercased so `Cmd+B` and `Cmd+Shift+B` differ only by the shift
    /// modifier, never by letter case.
    pub fn ch(c: char, mods: Mods) -> Self {
        Self {
            key: KeyRef::Char(c.to_ascii_lowercase()),
            mods,
        }
    }
}

/// Workspace keybinding table: chord -> command.
///
/// The keymap is consulted only for chords the focused panel did not
/// consume (panel-first capture). Lookups are exact-match on the chord;
/// there is no prefix/sequence state yet.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: Vec<(Chord, Command)>,
}

impl Keymap {
    /// Build an empty keymap. Prefer [`Keymap::default`] for the shipped
    /// bindings.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Bind a chord to a command, replacing any existing binding for the
    /// same chord. Returns `&mut self` for fluent setup.
    pub fn bind(&mut self, chord: Chord, command: Command) -> &mut Self {
        if let Some(slot) = self.bindings.iter_mut().find(|(c, _)| *c == chord) {
            slot.1 = command;
        } else {
            self.bindings.push((chord, command));
        }
        self
    }

    /// Resolve a chord to its bound command, if any.
    pub fn resolve(&self, chord: Chord) -> Option<Command> {
        self.bindings
            .iter()
            .find(|(c, _)| *c == chord)
            .map(|(_, command)| *command)
    }

    /// Number of bound chords.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Convert an Iced keyboard event into a workspace [`Chord`], if it
/// represents a bindable key press.
pub fn chord_from_keyboard_event(event: &iced::keyboard::Event) -> Option<Chord> {
    let iced::keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        repeat,
        ..
    } = event
    else {
        return None;
    };

    if *repeat {
        return None;
    }

    let mods = Mods {
        command: modifiers.command(),
        shift: modifiers.shift(),
        alt: modifiers.alt(),
    };

    let key_ref = match key.as_ref() {
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => KeyRef::Backspace,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => KeyRef::Escape,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => KeyRef::Enter,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => KeyRef::ArrowUp,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => KeyRef::ArrowDown,
        iced::keyboard::Key::Character(_) => {
            let character = key.to_latin(*physical_key)?;
            KeyRef::Char(character.to_ascii_lowercase())
        }
        _ => return None,
    };

    Some(Chord { mods, key: key_ref })
}

impl Default for Keymap {
    /// The shipped workspace bindings:
    ///
    /// * `Cmd+1/2/3` — open the left / right / bottom dock.
    /// * `Cmd+D` — split the focused pane.
    /// * `Cmd+W` — close the active tab.
    fn default() -> Self {
        let mut keymap = Keymap::new();
        keymap
            .bind(Chord::ch('1', Mods::CMD), Command::OpenDock(DockSide::Left))
            .bind(
                Chord::ch('2', Mods::CMD),
                Command::OpenDock(DockSide::Right),
            )
            .bind(
                Chord::ch('3', Mods::CMD),
                Command::OpenDock(DockSide::Bottom),
            )
            .bind(Chord::ch('d', Mods::CMD), Command::SplitFocused)
            .bind(Chord::ch('w', Mods::CMD), Command::CloseActiveTab);
        keymap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_location::DockSide;

    #[test]
    fn bind_then_resolve_returns_command() {
        let mut keymap = Keymap::new();
        keymap.bind(Chord::ch('d', Mods::CMD), Command::SplitFocused);
        assert_eq!(
            keymap.resolve(Chord::ch('d', Mods::CMD)),
            Some(Command::SplitFocused)
        );
    }

    #[test]
    fn rebinding_a_chord_replaces_the_command() {
        let mut keymap = Keymap::new();
        keymap.bind(Chord::ch('w', Mods::CMD), Command::SplitFocused);
        keymap.bind(Chord::ch('w', Mods::CMD), Command::CloseActiveTab);
        assert_eq!(keymap.len(), 1);
        assert_eq!(
            keymap.resolve(Chord::ch('w', Mods::CMD)),
            Some(Command::CloseActiveTab)
        );
    }

    #[test]
    fn unbound_chord_resolves_to_none() {
        let keymap = Keymap::default();
        assert_eq!(keymap.resolve(Chord::ch('z', Mods::CMD)), None);
    }

    #[test]
    fn char_chords_are_case_insensitive() {
        assert_eq!(Chord::ch('D', Mods::CMD), Chord::ch('d', Mods::CMD));
    }

    #[test]
    fn shift_distinguishes_chords() {
        assert_ne!(Chord::ch('d', Mods::CMD), Chord::ch('d', Mods::CMD_SHIFT));
    }

    #[test]
    fn default_keymap_binds_all_shipped_commands() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.resolve(Chord::ch('1', Mods::CMD)),
            Some(Command::OpenDock(DockSide::Left))
        );
        assert_eq!(
            keymap.resolve(Chord::ch('2', Mods::CMD)),
            Some(Command::OpenDock(DockSide::Right))
        );
        assert_eq!(
            keymap.resolve(Chord::ch('3', Mods::CMD)),
            Some(Command::OpenDock(DockSide::Bottom))
        );
        assert_eq!(
            keymap.resolve(Chord::ch('d', Mods::CMD)),
            Some(Command::SplitFocused)
        );
        assert_eq!(
            keymap.resolve(Chord::ch('w', Mods::CMD)),
            Some(Command::CloseActiveTab)
        );
    }
}
