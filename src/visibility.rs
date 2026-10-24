//! Visibility cohorts are a public-API fallback, not native Space identifiers.
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct ContextTracker {
    members: BTreeMap<u64, BTreeSet<u64>>,
    current: u64,
    next: u64,
}
impl ContextTracker {
    pub fn resolve(&mut self, visible: &[u64], space_changed: bool) -> u64 {
        let visible: BTreeSet<_> = visible.iter().copied().collect();
        // Empty/partial transition snapshots must not erase the last known cohort.
        if visible.is_empty() {
            return 0;
        }
        let existing = self
            .members
            .get(&self.current)
            .filter(|known| !space_changed && !known.is_disjoint(&visible))
            .map(|_| self.current);
        let context = existing
            .or_else(|| {
                self.members
                    .iter()
                    .filter_map(|(id, known)| {
                        let common = known.intersection(&visible).count();
                        (common > 0).then_some((
                            *id,
                            common as f64 / (known.len() + visible.len() - common) as f64,
                        ))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|v| v.0)
            })
            .unwrap_or_else(|| {
                self.next += 1;
                self.next
            });
        self.members.entry(context).or_default().extend(visible);
        self.current = context;
        context
    }
    pub fn retain(&mut self, live: &BTreeSet<u64>) {
        for known in self.members.values_mut() {
            known.retain(|id| live.contains(id));
        }
    }
}

/// Visible windows on another monitor must not identify or merge this monitor's
/// Spaces. Allocate globally unique engine contexts for each display's cohorts.
#[derive(Default)]
pub struct DisplayContexts {
    trackers: BTreeMap<u32, ContextTracker>,
    contexts: BTreeMap<(u32, u64), u64>,
    next: u64,
}
impl DisplayContexts {
    pub fn resolve(&mut self, display: u32, visible: &[u64], space_changed: bool) -> u64 {
        let local = self
            .trackers
            .entry(display)
            .or_default()
            .resolve(visible, space_changed);
        *self.contexts.entry((display, local)).or_insert_with(|| {
            self.next += 1;
            self.next
        })
    }

    /// Forget destroyed windows and remove migrated windows from their former
    /// display's cohorts, while retaining off-Space windows on their own display.
    pub fn retain(&mut self, displays: &BTreeMap<u64, u32>) {
        for (display, tracker) in &mut self.trackers {
            let live = displays
                .iter()
                .filter_map(|(id, owner)| (owner == display).then_some(*id))
                .collect();
            tracker.retain(&live);
        }
    }
}
