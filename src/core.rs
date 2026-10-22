use crate::geometry::{Direction, Rect, Region, expose};
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
    pub snap: Option<Region>,
    pub restore_frame: Option<Rect>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
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
    Snap(Direction),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Focus(WindowId),
    Move(WindowId, Rect),
    Snap(WindowId, Rect),
}
#[derive(Default)]
pub struct Engine {
    pub windows: BTreeMap<WindowId, Window>,
    pub contexts: BTreeMap<u64, Context>,
    pub current: u64,
    pub bounds: BTreeMap<u32, Rect>,
    pub frozen: bool,
    pub gap: f64,
}
impl Engine {
    pub fn new(gap: f64) -> Self {
        Self {
            gap,
            ..Self::default()
        }
    }
    pub fn context(&self) -> Option<&Context> {
        self.contexts.get(&self.current)
    }
    pub fn focused(&self) -> Option<WindowId> {
        self.context()?.focused
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
            Command::Snap(dir) => {
                let Some(bounds) = self.bounds.get(&w.display).copied() else {
                    return vec![];
                };
                if dir == Direction::Down {
                    if let Some(frame) = w.restore_frame {
                        self.clear_snap(id);
                        let w = self.windows.get_mut(&id).unwrap();
                        w.snap = None;
                        w.restore_frame = None;
                        return vec![Effect::Snap(id, frame.accessible(bounds))];
                    }
                    return vec![];
                }
                let region = if dir == Direction::Up {
                    Region::Full
                } else {
                    w.snap.unwrap_or(Region::Full).cycle(dir)
                };
                let frame = region.rect(bounds);
                self.windows.get_mut(&id).unwrap().restore_frame =
                    w.restore_frame.or(Some(w.frame));
                self.clear_snap(id);
                self.windows.get_mut(&id).unwrap().snap = Some(region);
                return vec![Effect::Snap(id, frame)];
            }
        };
        target
            .filter(|t| *t != id)
            .map(|t| vec![Effect::Focus(t)])
            .unwrap_or_default()
    }
    pub fn clear_snap(&mut self, id: WindowId) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.snap = None;
        }
    }

    pub fn plan(&self) -> Vec<Effect> {
        if self.frozen {
            return vec![];
        }
        let Some(active) = self.focused().and_then(|id| self.windows.get(&id)) else {
            return vec![];
        };
        let Some(ctx) = self.context() else {
            return vec![];
        };
        let mut effects = vec![];
        let active_bounds = self
            .bounds
            .get(&active.display)
            .copied()
            .unwrap_or(active.frame);
        let active_frame = if active.mode == Mode::Magnetic {
            active.frame.accessible(active_bounds)
        } else {
            active.frame
        };
        for id in &ctx.order {
            let Some(w) = self.windows.get(id) else {
                continue;
            };
            if w.mode == Mode::Floating || w.display != active.display {
                continue;
            }
            // Explicit snapped rectangles stay owned by their snap until a direct edit.
            if w.snap.is_some() {
                continue;
            }
            let frame = if w.id == active.id {
                active_frame
            } else {
                expose(w.frame, active_frame, active_bounds, self.gap)
            };
            if !w.frame.near(frame) {
                effects.push(Effect::Move(w.id, frame));
            }
        }
        effects
    }
    pub fn observed_frame(&mut self, id: WindowId, frame: Rect, manual: bool) {
        if !frame.valid() {
            return;
        }
        if manual && self.windows.get(&id).is_some_and(|w| !w.frame.near(frame)) {
            self.clear_snap(id);
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
