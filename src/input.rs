use crate::{config::Binding, core::Command};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Modifiers {
    pub control: bool,
    pub option: bool,
    pub shift: bool,
    pub command: bool,
}
impl Modifiers {
    pub fn from_flags(flags: u64) -> Self {
        Self {
            control: flags & (1 << 18) != 0,
            option: flags & (1 << 19) != 0,
            shift: flags & (1 << 17) != 0,
            command: flags & (1 << 20) != 0,
        }
    }
    pub fn overlay(self, required: Self) -> bool {
        (!required.control || self.control)
            && (!required.option || self.option)
            && (!required.shift || self.shift)
            && (!required.command || self.command)
    }
}
pub fn route(bindings: &[Binding], key: u16, modifiers: Modifiers) -> Option<Command> {
    bindings
        .iter()
        .find(|b| b.keycode == key && b.modifiers == modifiers)
        .map(|b| b.action.command())
}

/// macOS owns all tiling geometry and previous-size history. Whalu only changes
/// the native menu command requested from the foreground app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeShortcut {
    pub keycode: u16,
    pub flags: u64,
}
impl NativeShortcut {
    pub const fn tiling(keycode: u16, arrange: bool) -> Self {
        Self {
            keycode,
            flags: (1 << 23) | (1 << 18) | if arrange { 1 << 17 } else { 0 },
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    Pass,
    Consume(Option<Command>),
    Native(NativeShortcut),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRoute {
    Pass,
    Consume(Option<Command>),
}

/// Back/forward presses belong to the overview only if it was visible at
/// button-down. Keep that ownership through release, even after dismissal.
#[derive(Default)]
pub struct OverviewButtons {
    pressed: [Option<bool>; 2],
}
impl OverviewButtons {
    pub fn route(&mut self, visible: bool, kind: u32, button: i64) -> ButtonRoute {
        let (index, command) = match button {
            3 => (0, Command::FocusNext),
            4 => (1, Command::FocusPrevious),
            _ => return ButtonRoute::Pass,
        };
        match kind {
            25 => {
                let first = self.pressed[index].is_none();
                let owned = *self.pressed[index].get_or_insert(visible);
                if owned {
                    ButtonRoute::Consume(first.then_some(command))
                } else {
                    ButtonRoute::Pass
                }
            }
            26 => {
                if self.pressed[index].take() == Some(true) {
                    ButtonRoute::Consume(None)
                } else {
                    ButtonRoute::Pass
                }
            }
            27 if self.pressed[index] == Some(true) => ButtonRoute::Consume(None),
            _ => ButtonRoute::Pass,
        }
    }
}

#[derive(Default)]
pub struct ShortcutRouter {
    held: std::collections::BTreeMap<u16, Command>,
    overview_keys: std::collections::BTreeSet<u16>,
    command_session: bool,
}
impl ShortcutRouter {
    /// Command alone opens nothing. After a switch, retain the overview between
    /// presses until Command is released, and through modifier-first key release.
    /// Use the same ownership for the overview and mouse-wheel interception.
    pub fn overview_held(&mut self, modifiers: Modifiers, required: Modifiers) -> bool {
        if !modifiers.command {
            self.command_session = false;
        }
        modifiers.overlay(required) || self.command_session || !self.overview_keys.is_empty()
    }

    pub fn key(&mut self, bindings: &[Binding], key: u16, flags: u64, down: bool) -> KeyRoute {
        // Keep the original mapping through repeat/release, even if modifiers
        // are released first or configuration is reloaded while a key is held.
        if down && self.held.get(&key) == Some(&Command::FocusRecent) {
            return KeyRoute::Consume(None);
        }
        let command = if down {
            self.held.get(&key).copied().or_else(|| {
                if flags & (1 << 23) != 0 && ![123, 124, 125, 126].contains(&key) {
                    return None;
                }
                let command = route(bindings, key, Modifiers::from_flags(flags))?;
                if [48, 50].contains(&key)
                    && flags & (1 << 20) != 0
                    && matches!(command, Command::FocusNext | Command::FocusPrevious)
                {
                    self.overview_keys.insert(key);
                    self.command_session = true;
                }
                self.held.insert(key, command);
                Some(command)
            })
        } else {
            self.overview_keys.remove(&key);
            self.held.remove(&key)
        };
        match command {
            Some(Command::Native(shortcut)) => KeyRoute::Native(shortcut),
            Some(command) => KeyRoute::Consume(down.then_some(command)),
            None => KeyRoute::Pass,
        }
    }
}
