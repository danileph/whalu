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
