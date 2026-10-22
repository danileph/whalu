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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Full,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}
impl Region {
    /// Exact fractions of the usable display; snapping never adds layout spacing.
    pub fn rect(self, b: Rect) -> Rect {
        let (x, y, w, h) = match self {
            Self::Full => (0., 0., 1., 1.),
            Self::Left => (0., 0., 0.5, 1.),
            Self::Right => (0.5, 0., 0.5, 1.),
            Self::TopLeft => (0., 0., 0.5, 0.5),
            Self::BottomLeft => (0., 0.5, 0.5, 0.5),
            Self::TopRight => (0.5, 0., 0.5, 0.5),
            Self::BottomRight => (0.5, 0.5, 0.5, 0.5),
        };
        Rect::new(
            b.x + b.width * x,
            b.y + b.height * y,
            b.width * w,
            b.height * h,
        )
    }
    pub fn cycle(self, dir: Direction) -> Self {
        match (dir, self) {
            (Direction::Left, Self::Left) => Self::TopLeft,
            (Direction::Left, Self::TopLeft) => Self::BottomLeft,
            (Direction::Right, Self::Right) => Self::TopRight,
            (Direction::Right, Self::TopRight) => Self::BottomRight,
            (Direction::Left, _) => Self::Left,
            (Direction::Right, _) => Self::Right,
            _ => Self::Full,
        }
    }
}

/// Position the app-accepted size using the requested snap's outer edges.
/// Minimum sizes may overflow the snap region, but must not overflow the display
/// when the actual window can fit there. No further automatic resize is needed.
pub fn constrained_snap(target: Rect, actual: Rect, bounds: Rect) -> Rect {
    let right = target.x > bounds.x + 1. && (target.right() - bounds.right()).abs() < 2.;
    let bottom = target.y > bounds.y + 1. && (target.bottom() - bounds.bottom()).abs() < 2.;
    Rect {
        x: if right {
            target.right() - actual.width
        } else {
            target.x
        },
        y: if bottom {
            target.bottom() - actual.height
        } else {
            target.y
        },
        width: actual.width,
        height: actual.height,
    }
    .accessible(bounds)
}

/// Background exposure with a displacement penalty and hysteresis. Position-only output.
pub fn expose(frame: Rect, active: Rect, bounds: Rect, gap: f64) -> Rect {
    if frame.intersection(active).area() < frame.area() * 0.25 {
        return frame;
    }
    let score = |r: Rect| {
        let visible = r.intersection(bounds);
        let exposed = visible.area() - visible.intersection(active).area();
        exposed
            - ((r.x - frame.x).abs() + (r.y - frame.y).abs()) * frame.width.min(frame.height) * 0.16
    };
    let mut best = frame;
    let mut best_score = score(frame) + frame.area() * 0.025;
    for (x, y) in [
        (active.x - frame.width - gap, frame.y),
        (active.right() + gap, frame.y),
        (frame.x, active.y - frame.height - gap),
        (frame.x, active.bottom() + gap),
        (bounds.x, frame.y),
        (bounds.right() - frame.width, frame.y),
        (frame.x, bounds.y),
        (frame.x, bounds.bottom() - frame.height),
    ] {
        let candidate = Rect { x, y, ..frame }.accessible(bounds);
        let s = score(candidate);
        if s > best_score {
            best = candidate;
            best_score = s;
        }
    }
    best
}
