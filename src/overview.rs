//! Selection-relative wheel geometry, independent of the native renderer.

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

impl Wheel {
    pub fn new(height: f64) -> Self {
        let available = (height / 2. - 66.).max(0.);
        let radius = (available / 100.).floor().clamp(1., 5.) as usize;
        Self {
            radius,
            extent: available.min(radius as f64 * 105.),
            exit_extent: height / 2. + 66.,
        }
    }

    /// Positive offsets are later entries, below the selected card in AppKit coordinates.
    /// One extra slot beyond each edge lets opaque cards travel fully off-screen.
    pub fn pose(&self, offset: f64) -> Pose {
        let distance = offset.abs() / self.radius as f64;
        let progress = distance.min(1.);
        let sign = offset.signum();
        let curve = progress.powf(1.25);
        let edge = (offset.abs() - self.radius as f64).clamp(0., 1.);
        Pose {
            x: 88. - 12. * curve,
            y: -sign
                * (self.extent * (progress * 0.95).sin() / 0.95_f64.sin()
                    + edge * (self.exit_extent - self.extent)),
            scale: 1. - 0.28 * curve,
            pitch: sign * 46_f64.to_radians() * progress,
            // Both ends turn right; only the vertical wheel pitch changes sign.
            yaw: 24_f64.to_radians() * progress,
            depth: -70. * curve,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_pose_is_centered_on_every_display_size() {
        for height in [240., 600., 800., 1400.] {
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
        for height in [400., 600., 800., 1400.] {
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
                assert!(previous.y - below.y > occupied);
                assert!(below.y.abs() + 52. * below.scale < height / 2.);
                previous = below;
            }
            assert!(wheel.pose(wheel.radius as f64 + 1.).y.abs() >= height / 2. + 65.);
        }
    }
}
