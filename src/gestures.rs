//! Continuous three-contact navigation. Frames produce a session, signed scrub
//! offsets, and an end/cancel event; window focus remains a main-thread concern.
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub id: i32,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureEvent {
    Begin(Axis),
    Update(i32),
    End { cancelled: bool },
}
pub struct SwipeRecognizer {
    started: Option<f64>,
    triple_started: Option<f64>,
    baseline: BTreeMap<i32, (f64, f64)>,
    blocked: bool,
    frames: usize,
    last_time: f64,
    step: f64,
    axis: Option<Axis>,
    offset: i32,
    edge_offset: i32,
    edge: Option<(i32, f64)>,
    edge_blocked: bool,
    last_position: Option<f64>,
}
impl Default for SwipeRecognizer {
    fn default() -> Self {
        Self {
            started: None,
            triple_started: None,
            baseline: BTreeMap::new(),
            blocked: false,
            frames: 0,
            last_time: 0.,
            step: 0.12,
            axis: None,
            offset: 0,
            edge_offset: 0,
            edge: None,
            edge_blocked: false,
            last_position: None,
        }
    }
}
impl SwipeRecognizer {
    pub fn new(step: f64) -> Self {
        Self {
            step: if step.is_finite() && step > 0. {
                step.clamp(0.02, 0.5)
            } else {
                0.12
            },
            ..Default::default()
        }
    }
    pub fn cancel(&mut self) -> Vec<GestureEvent> {
        self.blocked = true;
        self.axis
            .take()
            .map(|_| vec![GestureEvent::End { cancelled: true }])
            .unwrap_or_default()
    }
    pub fn feed(&mut self, time: f64, contacts: &[Contact]) -> Vec<GestureEvent> {
        if contacts.is_empty() {
            let cancelled = self.blocked
                || !time.is_finite()
                || (self.started.is_some() && time < self.last_time);
            let result = self
                .axis
                .map(|_| vec![GestureEvent::End { cancelled }])
                .unwrap_or_default();
            let step = self.step;
            *self = Self::new(step);
            return result;
        }
        if !time.is_finite() || (self.started.is_some() && time < self.last_time) {
            return self.cancel();
        }
        self.last_time = time;
        let started = *self.started.get_or_insert(time);
        if contacts.len() > 3
            || contacts.iter().any(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || !(0. ..=1.).contains(&p.x)
                    || !(0. ..=1.).contains(&p.y)
            })
        {
            return self.cancel();
        }
        if self.blocked || contacts.len() != 3 {
            return vec![];
        }
        let positions: BTreeMap<_, _> = contacts.iter().map(|p| (p.id, (p.x, p.y))).collect();
        if positions.len() != 3 {
            return self.cancel();
        }
        if self.triple_started.is_none() {
            // A third finger joining an existing scroll must not turn it into navigation.
            if time - started > 0.25 {
                return self.cancel();
            }
            self.triple_started = Some(time);
            self.baseline = positions.clone();
        }
        if !positions.keys().eq(self.baseline.keys()) {
            return self.cancel();
        }
        self.frames += 1;
        let movement: Vec<_> = self
            .baseline
            .iter()
            .map(|(id, (x, y))| {
                let (nx, ny) = positions[id];
                (nx - x, ny - y)
            })
            .collect();
        let dx = movement.iter().map(|p| p.0).sum::<f64>() / 3.;
        let dy = movement.iter().map(|p| p.1).sum::<f64>() / 3.;
        let mut events = vec![];
        if self.axis.is_none() {
            // Give normal four-finger landings time to reveal the fourth contact.
            if self.frames < 3 || time - self.triple_started.unwrap() < 0.075 {
                return vec![];
            }
            let axis = if dx.abs() > dy.abs() * 1.45 {
                Axis::Horizontal
            } else if dy.abs() > dx.abs() * 1.45 {
                Axis::Vertical
            } else {
                return vec![];
            };
            let along = match axis {
                Axis::Horizontal => dx,
                Axis::Vertical => dy,
            };
            if along.abs() < self.step * 0.3 {
                return vec![];
            }
            if movement.iter().any(|(x, y)| {
                let a = match axis {
                    Axis::Horizontal => *x,
                    Axis::Vertical => *y,
                };
                a * along <= 0. || (a - along).abs() > along.abs() * 0.6
            }) {
                return self.cancel();
            }
            self.axis = Some(axis);
            events.push(GestureEvent::Begin(axis));
        }
        let axis = self.axis.unwrap();
        let (along, across) = match axis {
            Axis::Horizontal => (dx, dy),
            Axis::Vertical => (dy, dx),
        };
        if movement.iter().any(|(x, y)| {
            let (a, b) = match axis {
                Axis::Horizontal => (*x, *y),
                Axis::Vertical => (*y, *x),
            };
            (a - along).abs() > self.step * 0.75 || (b - across).abs() > self.step * 0.75
        }) {
            return self.cancel();
        }
        // Vertical up chooses the previous window; horizontal right chooses the next app window.
        let position = match axis {
            Axis::Horizontal => dx,
            Axis::Vertical => -dy,
        };
        let mut offset = self.offset;
        // Schmitt thresholds prevent selection oscillation around cell boundaries.
        while position - offset as f64 * self.step > self.step * 0.65 {
            offset += 1;
        }
        while position - offset as f64 * self.step < -self.step * 0.65 {
            offset -= 1;
        }
        let previous = self.offset + self.edge_offset;
        self.offset = offset;
        // Holding near an edge keeps traversing a long list without lifting and
        // repositioning fingers. Moving back toward the center stops edge travel.
        let center = positions
            .values()
            .map(|(x, y)| match axis {
                Axis::Horizontal => *x,
                Axis::Vertical => *y,
            })
            .sum::<f64>()
            / 3.;
        let edge_dir = match axis {
            Axis::Horizontal if center > 0.88 => 1,
            Axis::Horizontal if center < 0.12 => -1,
            Axis::Vertical if center > 0.88 => -1,
            Axis::Vertical if center < 0.12 => 1,
            _ => 0,
        };
        let delta = self.last_position.map(|last| position - last).unwrap_or(0.);
        self.last_position = Some(position);
        if edge_dir == 0 {
            self.edge = None;
            self.edge_blocked = false;
        } else {
            if delta * (edge_dir as f64) < -0.003 {
                self.edge = None;
                self.edge_blocked = true;
            }
            if delta * (edge_dir as f64) > 0.003 {
                self.edge_blocked = false;
            }
            if !self.edge_blocked {
                match self.edge {
                    Some((direction, due)) if direction == edge_dir && time >= due => {
                        self.edge_offset += edge_dir;
                        self.edge = Some((edge_dir, time + 0.22));
                    }
                    Some((direction, _)) if direction == edge_dir => {}
                    _ => self.edge = Some((edge_dir, time + 0.4)),
                }
            }
        }
        let current = self.offset + self.edge_offset;
        if current != previous {
            events.push(GestureEvent::Update(current));
        }
        events
    }
}
