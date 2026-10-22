//! Stable candidate order and reversible selection for one uninterrupted swipe.
use crate::{
    core::{Engine, WindowId},
    gestures::Axis,
};
#[derive(Debug)]
pub struct ScrubSession {
    pub context: u64,
    pub original: Option<WindowId>,
    pub axis: Axis,
    targets: Vec<WindowId>,
    index: Option<usize>,
    remembered: Option<usize>,
    last_offset: i32,
}
impl ScrubSession {
    pub fn new(engine: &Engine, axis: Axis) -> Option<Self> {
        let ctx = engine.context()?;
        let anchor = ctx
            .focused
            .or(ctx.last_focused.filter(|id| ctx.order.contains(id)))
            .or(ctx.order.first().copied())?;
        let targets: Vec<_> = match axis {
            Axis::Vertical => ctx.order.clone(),
            Axis::Horizontal => {
                let app = &engine.windows.get(&anchor)?.app_id;
                ctx.order
                    .iter()
                    .copied()
                    .filter(|id| engine.windows.get(id).is_some_and(|w| &w.app_id == app))
                    .collect()
            }
        };
        if targets.is_empty() {
            return None;
        }
        let position = |id| targets.iter().position(|target| *target == id);
        let index = ctx.focused.and_then(position);
        let remembered = ctx
            .last_focused
            .filter(|id| ctx.order.contains(id))
            .and_then(position);
        Some(Self {
            context: engine.current,
            original: ctx.focused,
            axis,
            targets,
            index,
            remembered,
            last_offset: 0,
        })
    }
    pub fn update(&mut self, offset: i32) -> Option<WindowId> {
        let mut delta = offset.saturating_sub(self.last_offset);
        self.last_offset = offset;
        if delta == 0 {
            return None;
        }
        let before = self.index;
        let index = match self.index {
            Some(i) => i,
            None => {
                let seed =
                    self.remembered
                        .unwrap_or(if delta > 0 { 0 } else { self.targets.len() - 1 });
                delta -= delta.signum();
                seed
            }
        };
        let index = (index as i64 + delta as i64).clamp(0, self.targets.len() as i64 - 1) as usize;
        self.index = Some(index);
        (before != self.index).then_some(self.targets[index])
    }
    pub fn selected(&self) -> Option<WindowId> {
        self.index.map(|i| self.targets[i])
    }
}
