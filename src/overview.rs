//! Selection-relative wheel geometry, independent of the native renderer.

/// The outermost logical point of a full display, including menu bar and Dock.
/// Half-open bounds keep adjacent monitors from claiming the same pointer.
pub fn at_right_edge(bounds: crate::geometry::Rect, point: (f64, f64)) -> bool {
    bounds.valid()
        && point.0 >= bounds.right() - 1.
        && point.0 < bounds.right()
        && point.1 >= bounds.y
        && point.1 < bounds.bottom()
}

// The 82-point preview and its 12-point badge overhang, plus shadow clearance.
const CARD_HEIGHT: f64 = 100.;
const MIN_GAP: f64 = 16.;
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

pub struct Wheel {
    pub radius: usize,
    extent: f64,
    exit_extent: f64,
}

/// Provisional drag order. Commit only on a valid drop; cancellation is free.
pub struct ReorderDrag {
    pub original: Vec<u64>,
    pub window: u64,
    pub center: usize,
    pub destination: usize,
    height: f64,
    y: f64,
}
impl ReorderDrag {
    pub fn new(order: &[u64], window: u64, selected: Option<u64>, height: f64) -> Option<Self> {
        let destination = order.iter().position(|id| *id == window)?;
        Some(Self {
            original: order.to_vec(),
            window,
            center: selected
                .and_then(|id| order.iter().position(|entry| *entry == id))
                .unwrap_or(0),
            destination,
            height,
            y: height / 2.,
        })
    }
    /// Pointer y increases downward, in logical points within the overview.
    pub fn update(&mut self, y: f64) {
        self.y = y;
        let wheel = Wheel::new(self.height);
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
    pub fn new(height: f64) -> Self {
        // AppKit supplies logical points: Retina pixel density must not change
        // card size or spacing. Grow the working set with usable display height.
        let available = (height / 2. - EDGE_INSET).max(0.);
        let radius = (available / (CARD_HEIGHT * MEAN_SCALE + MIN_GAP)).floor() as usize;
        Self {
            radius,
            extent: if radius == 0 { 0. } else { available },
            exit_extent: height / 2. + EDGE_INSET,
        }
    }

    /// Positive offsets are later entries, below the selected card in AppKit coordinates.
    /// One extra slot beyond each edge lets opaque cards travel fully off-screen.
    pub fn pose(&self, offset: f64) -> Pose {
        let distance = offset.abs() / self.radius.max(1) as f64;
        let progress = distance.min(1.);
        let sign = offset.signum();
        let curve = progress.powf(CURVE_POWER);
        let edge = (offset.abs() - self.radius as f64).clamp(0., 1.);
        // Integrate the tapered card heights rather than compressing a sine
        // curve near the edges. Every slot keeps its badge clearance, including
        // with Reduce Motion, and spare room becomes evenly distributed spacing.
        let occupied = CARD_HEIGHT
            * self.radius as f64
            * (progress - TAPER * progress.powf(CURVE_POWER + 1.) / (CURVE_POWER + 1.));
        let spacing = self.extent - CARD_HEIGHT * self.radius as f64 * MEAN_SCALE;
        Pose {
            x: 88. - 12. * curve,
            y: -sign * (occupied + spacing * progress + edge * (self.exit_extent - self.extent)),
            scale: 1. - TAPER * curve,
            pitch: sign * 46_f64.to_radians() * progress,
            // Both ends turn right; only the vertical wheel pitch changes sign.
            yaw: 24_f64.to_radians() * progress,
            depth: -70. * curve,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn right_edge_respects_full_display_bounds_and_adjacent_monitors() {
        use crate::geometry::Rect;
        let left = Rect::new(-1920., -200., 1920., 1080.);
        let right = Rect::new(0., 0., 2560., 1440.);
        for point in [(-1., -200.), (-0.5, 879.)] {
            assert!(super::at_right_edge(left, point));
            assert!(!super::at_right_edge(right, point));
        }
        for point in [(-1.01, 100.), (0., 100.), (-1., -201.), (-1., 880.)] {
            assert!(!super::at_right_edge(left, point));
        }
        assert!(super::at_right_edge(right, (2559.5, 1439.5)));
        assert!(!super::at_right_edge(right, (2560., 100.)));
        assert!(!super::at_right_edge(right, (f64::NAN, 100.)));
        assert!(!super::at_right_edge(Rect::default(), (0., 0.)));
    }

    use super::*;

    #[test]
    fn drag_inserts_in_both_directions_without_mutating_original() {
        let order = [1, 2, 3, 4];
        let mut drag = ReorderDrag::new(&order, 2, Some(2), 800.).unwrap();
        let wheel = Wheel::new(800.);
        drag.update(400. - wheel.pose(2.).y);
        assert_eq!(drag.order(), [1, 3, 4, 2]);
        drag.update(400. - wheel.pose(-1.).y);
        assert_eq!(drag.order(), [2, 1, 3, 4]);
        assert_eq!(drag.original, order);
        assert!(ReorderDrag::new(&order, 99, None, 800.).is_none());
    }

    #[test]
    fn edge_scrolling_reaches_every_slot_and_reverses_at_all_display_heights() {
        for height in [132., 240., 800., 1400., 2500.] {
            let order: Vec<_> = (1..=40).collect();
            let mut drag = ReorderDrag::new(&order, 20, Some(20), height).unwrap();
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
            let wheel = Wheel::new(height);
            {
                let pose = wheel.pose(0.);
                assert_eq!(pose.y, 0.);
                assert_eq!(pose.scale, 1.);
                assert_eq!(pose.pitch, 0.);
                assert_eq!(pose.yaw, 0.);
            }
        }
    }

    #[test]
    fn wheel_progresses_symmetrically_without_overlapping_cards() {
        for height in [240., 400., 600., 800., 1400., 2160., 3440.] {
            let wheel = Wheel::new(height);
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
                // Include the icon extending below the card in the separation bound.
                let occupied = 100. * (previous.scale + below.scale) / 2.;
                assert!(previous.y - below.y >= occupied + MIN_GAP - 1e-9);
                assert!(below.y.abs() + 52. * below.scale < height / 2.);
                previous = below;
            }
            assert!(wheel.pose(wheel.radius as f64 + 1.).y.abs() >= height / 2. + 65.);
        }
    }

    #[test]
    fn tall_displays_use_the_available_height_without_a_card_limit() {
        let laptop = Wheel::new(800.);
        let desktop = Wheel::new(1400.);
        let portrait = Wheel::new(2500.);
        assert!(desktop.radius > 5);
        assert!(desktop.radius > laptop.radius);
        assert!(portrait.radius > desktop.radius);
        for wheel in [laptop, desktop, portrait] {
            let edge = wheel.pose(wheel.radius as f64);
            assert!((edge.y.abs() - wheel.extent).abs() < 1e-9);
        }
    }

    #[test]
    fn spacing_survives_resolution_thresholds_and_fractional_travel() {
        // Cross every slot-count boundary, including very short displays.
        for height in (132..=4000).step_by(2) {
            let height = height as f64;
            let wheel = Wheel::new(height);
            let limit = (wheel.radius * 4) as isize;
            for step in -limit - 4..=limit {
                let offset = step as f64 / 4.;
                let a = wheel.pose(offset);
                let b = wheel.pose(offset + 1.);
                let occupied = CARD_HEIGHT * (a.scale + b.scale) / 2.;
                assert!(a.y - b.y >= occupied + MIN_GAP - 1e-9);
            }
            if wheel.radius > 0 {
                let edge = wheel.pose(wheel.radius as f64);
                assert!(edge.y.abs() + 52. * edge.scale < height / 2.);
            }
        }
    }
}
