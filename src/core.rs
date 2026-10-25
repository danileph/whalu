use crate::{geometry::Rect, input::NativeShortcut};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type WindowId = u64;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Magnetic,
    Floating,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub display: u32,
    pub frame: Rect,
    pub mode: Mode,
    pub last_floating_frame: Option<Rect>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub display: Option<u32>,
    pub order: Vec<WindowId>,
    pub focused: Option<WindowId>,
    pub last_focused: Option<WindowId>,
    pub history: Vec<WindowId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    FocusNext,
    FocusPrevious,
    AppNext,
    AppPrevious,
    ToggleMode,
    StackUp,
    StackDown,
    MoveSpace(bool),
    Native(NativeShortcut),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Focus(WindowId),
}
#[derive(Default)]
pub struct Engine {
    pub windows: BTreeMap<WindowId, Window>,
    pub contexts: BTreeMap<u64, Context>,
    pub current: u64,
    pub bounds: BTreeMap<u32, Rect>,
    pub frozen: bool,
}
impl Engine {
    pub fn new(_legacy_gap: f64) -> Self {
        Self::default()
    }
    pub fn context(&self) -> Option<&Context> {
        self.contexts.get(&self.current)
    }
    pub fn focused(&self) -> Option<WindowId> {
        self.context()?.focused
    }
    pub fn display(&self) -> Option<u32> {
        self.context()?.display.or_else(|| {
            self.focused()
                .and_then(|id| self.windows.get(&id))
                .map(|window| window.display)
        })
    }
    pub fn set_display_visible(
        &mut self,
        display: u32,
        context: u64,
        ids: &[WindowId],
        focused: Option<WindowId>,
    ) {
        let visible: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| self.windows.get(id).is_some_and(|w| w.display == display))
            .collect();
        self.set_visible(context, &visible, focused);
        self.contexts.get_mut(&context).unwrap().display = Some(display);
    }
    pub fn set_visible(&mut self, context: u64, ids: &[WindowId], focused: Option<WindowId>) {
        self.current = context;
        let live: BTreeSet<_> = ids
            .iter()
            .copied()
            .filter(|id| self.windows.contains_key(id))
            .collect();
        let ctx = self.contexts.entry(context).or_default();
        ctx.history.retain(|id| self.windows.contains_key(id));
        for id in ids {
            if live.contains(id) && !ctx.history.contains(id) {
                ctx.history.push(*id);
            }
        }
        ctx.order = ctx
            .history
            .iter()
            .copied()
            .filter(|id| live.contains(id))
            .collect();
        ctx.focused = focused.filter(|id| live.contains(id));
        if let Some(id) = ctx.focused {
            ctx.last_focused = Some(id);
        }
    }
    pub fn focus(&mut self, id: WindowId) {
        let Some(ctx) = self.contexts.get_mut(&self.current) else {
            return;
        };
        if !ctx.order.contains(&id) {
            return;
        }
        ctx.focused = Some(id);
        ctx.last_focused = Some(id);
    }
    /// Move one visible window, preserving hidden history entries and native focus.
    pub fn reorder(&mut self, id: WindowId, destination: usize) -> bool {
        if self.frozen {
            return false;
        }
        let Some(ctx) = self.contexts.get_mut(&self.current) else {
            return false;
        };
        let Some(source) = ctx.order.iter().position(|entry| *entry == id) else {
            return false;
        };
        if destination >= ctx.order.len() || source == destination {
            return false;
        }
        ctx.order.remove(source);
        ctx.order.insert(destination, id);
        // Only replace visible slots: minimized/temporarily absent windows keep
        // their history positions, as they do for keyboard stack edits.
        let visible: BTreeSet<_> = ctx.order.iter().copied().collect();
        let mut reordered = ctx.order.iter();
        for entry in &mut ctx.history {
            if visible.contains(entry) {
                *entry = *reordered.next().unwrap();
            }
        }
        true
    }
    fn enter_unfocused(&self, cmd: Command) -> Vec<Effect> {
        let Some(ctx) = self.context() else {
            return vec![];
        };
        let remembered = ctx.last_focused.filter(|id| ctx.order.contains(id));
        let target = match cmd {
            Command::FocusNext | Command::AppNext => remembered.or(ctx.order.first().copied()),
            Command::FocusPrevious | Command::AppPrevious => {
                remembered.or(ctx.order.last().copied())
            }
            _ => None,
        };
        target.map(|id| vec![Effect::Focus(id)]).unwrap_or_default()
    }
    pub fn command(&mut self, cmd: Command) -> Vec<Effect> {
        if self.frozen {
            return vec![];
        }
        let Some(id) = self.focused() else {
            return self.enter_unfocused(cmd);
        };
        let Some(w) = self.windows.get(&id).cloned() else {
            return vec![];
        };
        let ctx = self.context().unwrap();
        let target = match cmd {
            Command::FocusNext | Command::FocusPrevious => {
                step(&ctx.order, id, matches!(cmd, Command::FocusNext))
            }
            Command::AppNext | Command::AppPrevious => {
                let list: Vec<_> = ctx
                    .order
                    .iter()
                    .copied()
                    .filter(|i| self.windows.get(i).is_some_and(|v| v.app_id == w.app_id))
                    .collect();
                step(&list, id, matches!(cmd, Command::AppNext))
            }
            Command::StackUp | Command::StackDown => {
                let ctx = self.contexts.get_mut(&self.current).unwrap();
                let index = ctx.order.iter().position(|window| *window == id).unwrap();
                let neighbor = if cmd == Command::StackDown {
                    index.checked_add(1).filter(|next| *next < ctx.order.len())
                } else {
                    index.checked_sub(1)
                };
                if let Some(neighbor) = neighbor {
                    let other = ctx.order[neighbor];
                    ctx.order.swap(index, neighbor);
                    let a = ctx.history.iter().position(|window| *window == id).unwrap();
                    let b = ctx
                        .history
                        .iter()
                        .position(|window| *window == other)
                        .unwrap();
                    ctx.history.swap(a, b);
                }
                return vec![];
            }
            Command::MoveSpace(_) => return vec![],
            Command::ToggleMode => {
                let w = self.windows.get_mut(&id).unwrap();
                w.mode = match w.mode {
                    Mode::Magnetic => {
                        w.last_floating_frame = Some(w.frame);
                        Mode::Floating
                    }
                    Mode::Floating => Mode::Magnetic,
                };
                return vec![];
            }
            // Native menu actions run in the adapter, never as geometry effects.
            Command::Native(_) => return vec![],
        };
        target
            .filter(|t| *t != id)
            .map(|t| vec![Effect::Focus(t)])
            .unwrap_or_default()
    }
    pub fn observed_frame(&mut self, id: WindowId, frame: Rect, _manual: bool) {
        if !frame.valid() {
            return;
        }
        if let Some(w) = self.windows.get_mut(&id) {
            w.frame = frame;
            if w.mode == Mode::Floating {
                w.last_floating_frame = Some(frame);
            }
        }
    }
}
fn step(ids: &[u64], id: u64, forward: bool) -> Option<u64> {
    let i = ids.iter().position(|v| *v == id)?;
    Some(
        ids[if forward {
            (i + 1) % ids.len()
        } else {
            (i + ids.len() - 1) % ids.len()
        }],
    )
}
