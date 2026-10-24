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
#[derive(Default)]
pub struct ShortcutRouter {
    held: std::collections::BTreeMap<u16, Command>,
}
impl ShortcutRouter {
    pub fn key(&mut self, bindings: &[Binding], key: u16, flags: u64, down: bool) -> KeyRoute {
        // Keep the original mapping through repeat/release, even if modifiers
        // are released first or configuration is reloaded while a key is held.
        let command = if down {
            self.held.get(&key).copied().or_else(|| {
                if flags & (1 << 23) != 0 && ![123, 124, 125, 126].contains(&key) {
                    return None;
                }
                let command = route(bindings, key, Modifiers::from_flags(flags))?;
                self.held.insert(key, command);
                Some(command)
            })
        } else {
            self.held.remove(&key)
        };
        match command {
            Some(Command::Native(shortcut)) => KeyRoute::Native(shortcut),
            Some(command) => KeyRoute::Consume(down.then_some(command)),
            None => KeyRoute::Pass,
        }
    }
}
