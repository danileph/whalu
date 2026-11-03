//! Selection-relative wheel geometry, independent of the native renderer.

/// The outermost logical point of a full display, including menu bar and Dock.
/// Half-open bounds keep adjacent monitors from claiming the same pointer.
pub fn at_left_edge(bounds: crate::geometry::Rect, point: (f64, f64)) -> bool {
    bounds.valid()
        && point.0 >= bounds.x
        && point.0 < bounds.x + 1.
        && point.1 >= bounds.y
        && point.1 < bounds.bottom()
}

// The 82-point preview and its 12-point badge overhang, plus shadow clearance.
const CARD_HEIGHT: f64 = 100.;
const CAPACITY_GAP: f64 = 16.;
const PERSPECTIVE: f64 = 600.;
const EDGE_INSET: f64 = 66.;
const TAPER: f64 = 0.28;
const CURVE_POWER: f64 = 1.25;
const MEAN_SCALE: f64 = 1. - TAPER / (CURVE_POWER + 1.);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub depth: f64,
}

impl Pose {
    /// Project a card-local point around its (64, 41) center, matching the
    /// renderer's scale, yaw, pitch, depth and perspective in that order.
    pub fn projected_y(&self, x: f64, y: f64, reduce_motion: bool) -> f64 {
        let x = (x - 64.) * self.scale;
        let y = (y - 41.) * self.scale;
        if reduce_motion {
            return y;
        }
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        let yaw_sin = self.yaw.sin();
        let rotated_y = y * pitch_cos + x * yaw_sin * pitch_sin;
        let z = self.depth + y * pitch_sin - x * yaw_sin * pitch_cos;
        rotated_y / (1. - z / PERSPECTIVE)
    }

    /// Vertical silhouette of the preview body, relative to its projected center.
    pub fn vertical_bounds(&self, reduce_motion: bool) -> (f64, f64) {
        [(0., 0.), (128., 0.), (0., 82.), (128., 82.)]
            .into_iter()
            .map(|(x, y)| self.projected_y(x, y, reduce_motion))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| {
                (min.min(y), max.max(y))
            })
    }
}

pub struct Wheel {
    pub radius: usize,
    positions: Vec<f64>,
    /// Space reserved for compact icons at each end, in display points.
    pub overflow_height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct OverflowIcon {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

/// Provisional drag order. Commit only on a valid drop; cancellation is free.
pub struct ReorderDrag {
    pub original: Vec<u64>,
    pub window: u64,
    pub center: usize,
    pub destination: usize,
    height: f64,
    reduce_motion: bool,
    y: f64,
}
impl ReorderDrag {
    pub fn new(
        order: &[u64],
        window: u64,
        selected: Option<u64>,
        height: f64,
        reduce_motion: bool,
    ) -> Option<Self> {
        let destination = order.iter().position(|id| *id == window)?;
        Some(Self {
            original: order.to_vec(),
            window,
            center: selected
                .and_then(|id| order.iter().position(|entry| *entry == id))
                .unwrap_or(0),
            destination,
            height,
            reduce_motion,
            y: height / 2.,
        })
    }
    /// Pointer y increases downward, in logical points within the overview.
    pub fn update(&mut self, y: f64) {
        self.y = y;
        let wheel = Wheel::for_windows(self.height, self.original.len(), self.reduce_motion);
        let start = self.center.saturating_sub(wheel.radius);
        let end = (self.center + wheel.radius + 1).min(self.original.len());
        self.destination = (start..end)
            .min_by(|a, b| {
                let distance = |i: usize| {
                    (self.height / 2. - wheel.pose(i as f64 - self.center as f64).y - y).abs()
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .unwrap_or(self.center);
    }
    pub fn edge_direction(&self) -> i32 {
        if self.y < 48. && self.center > 0 {
            -1
        } else if self.y > self.height - 48. && self.center + 1 < self.original.len() {
            1
        } else {
            0
        }
    }
    pub fn scroll(&mut self) {
        self.center = (self.center as i64 + self.edge_direction() as i64) as usize;
        self.update(self.y);
    }
    pub fn order(&self) -> Vec<u64> {
        let mut order = self.original.clone();
        order.retain(|id| *id != self.window);
        order.insert(self.destination, self.window);
        order
    }
}

impl Wheel {
    pub fn new(height: f64, reduce_motion: bool) -> Self {
        Self::with_inset(height, EDGE_INSET, reduce_motion)
    }

    /// Reserve compact rows only when the stack exceeds the full thumbnail set.
    /// Use the worst-case end of the stack so navigation never changes capacity.
    pub fn for_windows(height: f64, count: usize, reduce_motion: bool) -> Self {
        let mut wheel = Self::new(height, reduce_motion);
        if count <= wheel.radius + 1 {
            return wheel;
        }
        loop {
            let rows = count.saturating_sub(wheel.radius + 1).div_ceil(3).min(3);
            let inset = (48. + rows as f64 * 32.).min(height / 4.).max(EDGE_INSET);
            let mut next = Self::with_inset(height, inset, reduce_motion);
            next.overflow_height = (inset - 40.).min((height / 2. - 56.).max(0.));
            if next.radius == wheel.radius {
                return next;
            }
            wheel = next;
        }
    }

    /// The first hidden window stays at the badge column. New hidden windows
    /// push onto the right end; reversing navigation pops the newest entry first.
    pub fn overflow(&self, height: f64, count: usize, selected: usize) -> Vec<OverflowIcon> {
        let start = selected.saturating_sub(self.radius);
        let end = (selected + self.radius + 1).min(count);
        let mut icons = Vec::with_capacity(start + count - end);
        let anchor = self.pose(self.radius as f64).x;
        let width = 168. - anchor;
        for (range, above) in [(0..start, true), (end..count, false)] {
            let count = range.len();
            if count == 0 {
                continue;
            }
            let columns = ((count as f64 * (width + 14.) / self.overflow_height)
                .sqrt()
                .ceil() as usize)
                .max(3);
            let rows = count.div_ceil(columns);
            let step = (width / (columns as f64 - 0.5))
                .min(self.overflow_height / rows as f64)
                .min(32.);
            for slot in 0..count {
                let index = if above { slot } else { range.end - slot - 1 };
                let row = slot / columns;
                let column = slot % columns;
                let size = step * 28. / 32.;
                let x = anchor - size / 2. + column as f64 * step;
                let top = if above {
                    height - (self.overflow_height - rows as f64 * step) / 2.
                } else {
                    (self.overflow_height + rows as f64 * step) / 2.
                };
                icons.push(OverflowIcon {
                    index,
                    x,
                    y: top - (row + 1) as f64 * step + (step - size) / 2.,
                    size,
                });
            }
        }
        icons
    }

    fn with_inset(height: f64, inset: f64, reduce_motion: bool) -> Self {
        // Keep the working set based on logical display height, independently
        // of Retina density or the system motion preference.
        let available = (height / 2. - inset).max(0.);
        let radius = (available / (CARD_HEIGHT * MEAN_SCALE + CAPACITY_GAP)).floor() as usize;
        let mut wheel = Self {
            radius,
            positions: vec![0.; radius + 2],
            overflow_height: 0.,
        };
        if radius > 0 {
            let mut slots = Vec::with_capacity(radius);
            for i in 1..=radius {
                let previous = wheel.shape((i - 1) as f64).vertical_bounds(reduce_motion);
                let current = wheel.shape(i as f64).vertical_bounds(reduce_motion);
                let midpoint = wheel.shape(i as f64 - 0.5);
                let weight = if reduce_motion {
                    midpoint.scale
                } else {
                    midpoint.scale * midpoint.pitch.cos() / (1. - midpoint.depth / PERSPECTIVE)
                };
                slots.push((current.1 - previous.0, weight));
            }
            // Allocate free space between the actual projected thumbnail
            // edges. Rotation must not turn a shrinking layout gap into more
            // visible whitespace. Gap weights recede with scale and tilt.
            let occupied: f64 = slots.iter().map(|slot| slot.0).sum();
            let weights: f64 = slots.iter().map(|slot| slot.1).sum();
            let gap = (available - occupied) / weights;
            for (i, (occupied, weight)) in slots.into_iter().enumerate() {
                wheel.positions[i + 1] = wheel.positions[i] + occupied + gap * weight;
            }
        }
        wheel.positions[radius + 1] = height / 2. + EDGE_INSET;
        wheel
    }

    fn shape(&self, offset: f64) -> Pose {
        let progress = (offset.abs() / self.radius.max(1) as f64).min(1.);
        let curve = progress.powf(CURVE_POWER);
        Pose {
            x: 88. - 12. * curve,
            y: 0.,
            scale: 1. - TAPER * curve,
            pitch: offset.signum() * 46_f64.to_radians() * progress,
            // Both ends turn right; only the vertical wheel pitch changes sign.
            yaw: 24_f64.to_radians() * progress,
            depth: -70. * curve,
        }
    }

    /// Positive offsets are later entries, below the selected card in AppKit coordinates.
    /// One extra slot beyond each edge lets opaque cards travel fully off-screen.
    pub fn pose(&self, offset: f64) -> Pose {
        let distance = offset.abs().min((self.radius + 1) as f64);
        let index = (distance.floor() as usize).min(self.radius);
        let fraction = distance - index as f64;
        let y =
            self.positions[index] + (self.positions[index + 1] - self.positions[index]) * fraction;
        Pose {
            y: -offset.signum() * y,
            ..self.shape(offset)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn left_edge_respects_full_display_bounds_and_adjacent_monitors() {
        use crate::geometry::Rect;
        let left = Rect::new(-1920., -200., 1920., 1080.);
        let right = Rect::new(0., 0., 2560., 1440.);
        for point in [(-1920., -200.), (-1919.5, 879.)] {
            assert!(super::at_left_edge(left, point));
            assert!(!super::at_left_edge(right, point));
        }
        for point in [
            (-1920.01, 100.),
            (-1919., 100.),
            (-1920., -201.),
            (-1920., 880.),
        ] {
            assert!(!super::at_left_edge(left, point));
        }
        assert!(super::at_left_edge(right, (0.5, 1439.5)));
        assert!(!super::at_left_edge(right, (1., 100.)));
        assert!(super::at_left_edge(right, (0., 100.)));
        assert!(!super::at_left_edge(left, (0., 100.)));
        assert!(!super::at_left_edge(right, (2559.5, 100.)));
        assert!(!super::at_left_edge(right, (f64::NAN, 100.)));
        assert!(!super::at_left_edge(Rect::default(), (0., 0.)));
    }

    use super::*;

    #[test]
    fn overflow_represents_each_hidden_window_without_covering_thumbnails() {
        for reduced in [false, true] {
            for height in [132., 240., 400., 787., 800., 1400., 3440.] {
                for count in [1, 2, 4, 7, 10, 15, 40, 100, 1000] {
                    let wheel = Wheel::for_windows(height, count, reduced);
                    for selected in [0, count / 2, count - 1] {
                        let icons = wheel.overflow(height, count, selected);
                        let start = selected.saturating_sub(wheel.radius);
                        let end = (selected + wheel.radius + 1).min(count);
                        let represented: Vec<_> = icons.iter().map(|icon| icon.index).collect();
                        assert_eq!(
                            represented,
                            (0..start).chain((end..count).rev()).collect::<Vec<_>>()
                        );
                        for icon in &icons {
                            assert!(icon.size > 0. && icon.size <= 28.);
                            assert!(icon.x >= 16. && icon.x + icon.size <= 168.);
                            let (low, high) = if icon.index < start {
                                (height - wheel.overflow_height, height)
                            } else {
                                (0., wheel.overflow_height)
                            };
                            assert!(icon.y >= low - 1e-9);
                            assert!(icon.y + icon.size <= high + 1e-9);
                        }
                        for first in [icons.first(), icons.get(start)].into_iter().flatten() {
                            assert!(
                                (first.x + first.size / 2. - wheel.pose(wheel.radius as f64).x)
                                    .abs()
                                    < 1e-9
                            );
                        }
                        for pair in icons.windows(2) {
                            let [a, b] = pair else { unreachable!() };
                            assert!(
                                a.x + a.size <= b.x + 1e-9
                                    || b.y + b.size <= a.y + 1e-9
                                    || a.y + a.size <= b.y + 1e-9
                            );
                        }
                        for index in start..end {
                            let pose = wheel.pose(index as f64 - selected as f64);
                            let (min, max) = bounds_with_badge(pose, reduced);
                            assert!(height / 2. + pose.y + min >= wheel.overflow_height);
                            assert!(height / 2. + pose.y + max <= height - wheel.overflow_height);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hidden_stacks_push_right_and_pop_the_newest_window_in_both_directions() {
        for reduced in [false, true] {
            for height in [132., 400., 787., 1400.] {
                let count = 40;
                let wheel = Wheel::for_windows(height, count, reduced);
                let stacks = |selected: usize| {
                    let icons = wheel.overflow(height, count, selected);
                    let (above, below): (Vec<_>, Vec<_>) = icons
                        .iter()
                        .map(|icon| icon.index)
                        .partition(|index| *index < selected);
                    (above, below)
                };
                for selected in 0..count - 1 {
                    let (above, below) = stacks(selected);
                    let (next_above, next_below) = stacks(selected + 1);
                    if selected >= wheel.radius {
                        assert_eq!(next_above.last(), Some(&(selected - wheel.radius)));
                        assert_eq!(&next_above[..next_above.len() - 1], above);
                    }
                    if !below.is_empty() {
                        assert_eq!(below.last(), Some(&(selected + wheel.radius + 1)));
                        assert_eq!(&below[..below.len() - 1], next_below);
                    }
                }
            }
        }
    }

    #[test]
    fn drag_inserts_in_both_directions_without_mutating_original() {
        let order = [1, 2, 3, 4];
        let mut drag = ReorderDrag::new(&order, 2, Some(2), 800., false).unwrap();
        let wheel = Wheel::for_windows(800., order.len(), false);
        drag.update(400. - wheel.pose(2.).y);
        assert_eq!(drag.order(), [1, 3, 4, 2]);
        drag.update(400. - wheel.pose(-1.).y);
        assert_eq!(drag.order(), [2, 1, 3, 4]);
        assert_eq!(drag.original, order);
        assert!(ReorderDrag::new(&order, 99, None, 800., false).is_none());
    }

    #[test]
    fn drag_targets_match_projected_slots_in_both_motion_modes() {
        let order: Vec<_> = (1..=40).collect();
        for reduce_motion in [false, true] {
            for height in [132., 400., 787., 1400., 3440.] {
                let wheel = Wheel::for_windows(height, order.len(), reduce_motion);
                for center in [0, 20, 39] {
                    let mut drag = ReorderDrag::new(
                        &order,
                        order[center],
                        Some(order[center]),
                        height,
                        reduce_motion,
                    )
                    .unwrap();
                    let start = center.saturating_sub(wheel.radius);
                    let end = (center + wheel.radius + 1).min(order.len());
                    for target in start..end {
                        let pose = wheel.pose(target as f64 - center as f64);
                        drag.update(height / 2. - pose.y);
                        assert_eq!(drag.destination, target);
                    }
                }
            }
        }
    }

    #[test]
    fn edge_scrolling_reaches_every_slot_and_reverses_at_all_display_heights() {
        for height in [132., 240., 800., 1400., 2500.] {
            let order: Vec<_> = (1..=40).collect();
            let mut drag = ReorderDrag::new(&order, 20, Some(20), height, false).unwrap();
            drag.update(height - 1.);
            for _ in 0..50 {
                drag.scroll();
            }
            assert_eq!(drag.destination, 39);
            assert_eq!(drag.edge_direction(), 0);
            assert_eq!(drag.order().last(), Some(&20));
            drag.update(1.);
            for _ in 0..50 {
                drag.scroll();
            }
            assert_eq!(drag.destination, 0);
            assert_eq!(drag.edge_direction(), 0);
            assert_eq!(drag.order().first(), Some(&20));
            drag.update(height / 2.);
            assert_eq!(drag.edge_direction(), 0);
        }
    }

    #[test]
    fn selection_pose_is_centered_on_every_display_size() {
        for height in [132., 240., 600., 800., 1400., 2160., 3440.] {
            let wheel = Wheel::new(height, false);
            {
                let pose = wheel.pose(0.);
                assert_eq!(pose.y, 0.);
                assert_eq!(pose.scale, 1.);
                assert_eq!(pose.pitch, 0.);
                assert_eq!(pose.yaw, 0.);
            }
        }
    }

    fn bounds_with_badge(pose: Pose, reduce_motion: bool) -> (f64, f64) {
        let (mut min, mut max) = pose.vertical_bounds(reduce_motion);
        for (x, y) in [(50., -12.), (78., -12.), (50., 16.), (78., 16.)] {
            let projected = pose.projected_y(x, y, reduce_motion);
            min = min.min(projected);
            max = max.max(projected);
        }
        (min, max)
    }

    fn visible_gap(a: Pose, b: Pose, reduce_motion: bool) -> f64 {
        a.y + a.vertical_bounds(reduce_motion).0 - b.y - b.vertical_bounds(reduce_motion).1
    }

    #[test]
    fn wheel_progresses_symmetrically_without_overlapping_cards() {
        for reduce_motion in [false, true] {
            for height in [240., 400., 600., 800., 1400., 2160., 3440.] {
                let wheel = Wheel::new(height, reduce_motion);
                let mut previous = wheel.pose(0.);
                for i in 1..=wheel.radius {
                    let below = wheel.pose(i as f64);
                    let above = wheel.pose(-(i as f64));
                    assert_eq!(above.y, -below.y);
                    assert_eq!(above.pitch, -below.pitch);
                    assert_eq!(above.yaw, below.yaw);
                    assert!(above.yaw > 0.);
                    assert_eq!(above.scale, below.scale);
                    assert!(below.scale < previous.scale);
                    assert!(below.pitch > previous.pitch);
                    assert!(below.yaw.abs() > previous.yaw.abs());
                    assert!(below.depth < previous.depth);
                    let gap = previous.y + bounds_with_badge(previous, reduce_motion).0
                        - below.y
                        - bounds_with_badge(below, reduce_motion).1;
                    assert!(gap >= 4., "badge clearance: {gap}");
                    assert!(below.y.abs() + 52. * below.scale < height / 2.);
                    previous = below;
                }
                assert!(wheel.pose(wheel.radius as f64 + 1.).y.abs() >= height / 2. + 65.);
            }
        }
    }

    #[test]
    fn tall_displays_use_the_available_height_without_a_card_limit() {
        for reduce_motion in [false, true] {
            let laptop = Wheel::new(800., reduce_motion);
            let desktop = Wheel::new(1400., reduce_motion);
            let portrait = Wheel::new(2500., reduce_motion);
            assert!(desktop.radius > 5);
            assert!(desktop.radius > laptop.radius);
            assert!(portrait.radius > desktop.radius);
            for (wheel, height) in [(laptop, 800.), (desktop, 1400.), (portrait, 2500.)] {
                let edge = wheel.pose(wheel.radius as f64);
                assert!((edge.y.abs() - (height / 2. - EDGE_INSET)).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn visible_edge_gaps_shrink_toward_both_ends_with_and_without_rotation() {
        for reduce_motion in [false, true] {
            for height in [600., 787., 800., 1400., 2160., 3440.] {
                let wheel = Wheel::new(height, reduce_motion);
                for fraction in [0., 0.25, 0.5, 0.75] {
                    let mut previous_gap = f64::INFINITY;
                    for i in 0..wheel.radius {
                        let offset = i as f64 + fraction;
                        if offset + 1. > wheel.radius as f64 {
                            break;
                        }
                        let gap =
                            visible_gap(wheel.pose(offset), wheel.pose(offset + 1.), reduce_motion);
                        let above = visible_gap(
                            wheel.pose(-offset - 1.),
                            wheel.pose(-offset),
                            reduce_motion,
                        );
                        assert!(
                            gap < previous_gap,
                            "height {height}, offset {offset}, reduced {reduce_motion}: {gap} >= {previous_gap}"
                        );
                        assert!((above - gap).abs() < 1e-9);
                        previous_gap = gap;
                    }
                }
            }
        }
    }

    #[test]
    fn spacing_survives_resolution_thresholds_and_fractional_travel() {
        // Cross every slot-count boundary, including very short displays.
        for reduce_motion in [false, true] {
            for height in (132..=4000).step_by(2) {
                let height = height as f64;
                let wheel = Wheel::new(height, reduce_motion);
                let limit = (wheel.radius * 4) as isize;
                for step in -limit - 4..=limit {
                    let offset = step as f64 / 4.;
                    let a = wheel.pose(offset);
                    let b = wheel.pose(offset + 1.);
                    let gap = a.y + bounds_with_badge(a, reduce_motion).0
                        - b.y
                        - bounds_with_badge(b, reduce_motion).1;
                    assert!(
                        gap >= 4.,
                        "height {height}, offset {offset}, reduced {reduce_motion}, gap {gap}"
                    );
                }
                if wheel.radius > 0 {
                    let edge = wheel.pose(wheel.radius as f64);
                    assert!(edge.y.abs() + 52. * edge.scale < height / 2.);
                }
            }
        }
    }
}
