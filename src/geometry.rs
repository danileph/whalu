use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn right(self) -> f64 {
        self.x + self.width
    }
    pub fn bottom(self) -> f64 {
        self.y + self.height
    }
    pub fn center(self) -> (f64, f64) {
        (self.x + self.width / 2., self.y + self.height / 2.)
    }
    pub fn area(self) -> f64 {
        self.width.max(0.) * self.height.max(0.)
    }
    pub fn intersection(self, b: Self) -> Self {
        Self::new(
            self.x.max(b.x),
            self.y.max(b.y),
            (self.right().min(b.right()) - self.x.max(b.x)).max(0.),
            (self.bottom().min(b.bottom()) - self.y.max(b.y)).max(0.),
        )
    }
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.
            && self.height > 0.
    }
    pub fn near(self, b: Self) -> bool {
        [
            self.x - b.x,
            self.y - b.y,
            self.width - b.width,
            self.height - b.height,
        ]
        .iter()
        .all(|v| v.abs() < 2.)
    }
    /// Preserve dimensions, including oversized windows; keep the title bar reachable.
    pub fn accessible(self, bounds: Self) -> Self {
        let max_x = (bounds.right() - self.width).max(bounds.x);
        let max_y = (bounds.bottom() - self.height).max(bounds.y);
        Self {
            x: self.x.clamp(bounds.x, max_x),
            y: self.y.clamp(bounds.y, max_y),
            ..self
        }
    }
}
