//! Stable stack order for continuous swipes and sequential thumbnail clicks.
use crate::{
    core::{Engine, WindowId},
    gestures::Axis,
};
use std::collections::{BTreeSet, VecDeque};

/// Clicks traverse adjacent windows with the same native focus/raise steps as
/// keyboard and swipe navigation. Stop if focus no longer matches the last step.
pub struct ClickSequence {
    context: u64,
    target: WindowId,
    selected: Option<WindowId>,
    remaining: VecDeque<WindowId>,
}
impl ClickSequence {
    pub fn new(engine: &Engine, target: WindowId) -> Option<Self> {
        Self::from_selection(engine, target, engine.focused())
    }
    pub fn from_selection(
        engine: &Engine,
        target: WindowId,
        selected: Option<WindowId>,
    ) -> Option<Self> {
        let ctx = engine.context()?;
        let end = ctx.order.iter().position(|id| *id == target)?;
        let start = selected.and_then(|id| ctx.order.iter().position(|entry| *entry == id));
        let remaining = match start {
            Some(start) if start < end => ctx.order[start + 1..=end].iter().copied().collect(),
            Some(start) if start > end => ctx.order[end..start].iter().rev().copied().collect(),
            Some(_) => VecDeque::new(),
            None => {
                // A newly entered Space can have no local AX focus. Enter its
                // remembered window (or first entry) before traversing, just as
                // keyboard/swipe navigation does, rather than jumping to target.
                let anchor = ctx
                    .last_focused
                    .and_then(|id| ctx.order.iter().position(|entry| *entry == id))
                    .unwrap_or(0);
                if anchor <= end {
                    ctx.order[anchor..=end].iter().copied().collect()
                } else {
                    ctx.order[end..=anchor].iter().rev().copied().collect()
                }
            }
        };
        Some(Self {
            context: engine.current,
            target,
            selected,
            remaining,
        })
    }
    /// Quiet raises are reliable only for independent applications along the
    /// whole path. Repeated process or known app identities retain traversal.
    pub fn background_windows(
        &self,
        owner: impl Fn(WindowId) -> Option<(i32, Option<String>)>,
    ) -> Option<Vec<WindowId>> {
        let (target_pid, target_bundle) = owner(self.target)?;
        let windows: Vec<_> = self
            .remaining
            .iter()
            .copied()
            .filter(|id| *id != self.target)
            .collect();
        if windows.is_empty() {
            return None;
        }
        let mut processes = BTreeSet::from([target_pid]);
        let mut bundles: BTreeSet<_> = target_bundle.into_iter().collect();
        let mut independent = |id| {
            owner(id).is_some_and(|(pid, bundle)| {
                processes.insert(pid) && bundle.is_none_or(|bundle| bundles.insert(bundle))
            })
        };
        if self
            .selected
            .filter(|id| *id != self.target)
            .is_some_and(|id| !independent(id))
        {
            return None;
        }
        windows.iter().all(|id| independent(*id)).then_some(windows)
    }
    /// The destination was accepted immediately. Consume the focus path so a
    /// failed background repair cannot replay intermediate foreground windows.
    pub fn accept_destination(&mut self) {
        self.selected = Some(self.target);
        self.remaining.clear();
    }
    pub fn advance(&mut self, engine: &Engine) -> Option<WindowId> {
        let valid = self.context == engine.current
            && engine.context().is_some_and(|ctx| {
                ctx.focused == self.selected
                    && self
                        .remaining
                        .iter()
                        .all(|id| ctx.order.contains(id) && engine.windows.contains_key(id))
            });
        if !valid {
            self.remaining.clear();
            return None;
        }
        let next = self.remaining.pop_front()?;
        self.selected = Some(next);
        Some(next)
    }
    pub fn pending(&self) -> bool {
        !self.remaining.is_empty()
    }
    pub fn selected(&self) -> Option<WindowId> {
        self.selected
    }
    pub fn target(&self) -> WindowId {
        self.target
    }
    pub fn context(&self) -> u64 {
        self.context
    }
}

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
        let targets = ctx.order.clone();
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
