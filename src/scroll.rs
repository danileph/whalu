//! Portable scroll ownership and bounded kinetic navigation.

/// Own only a three-contact scroll stream and its trailing native momentum.
/// A fresh ordinary touch sequence immediately releases the previous stream.
#[derive(Default)]
pub struct ScrollGuard {
    candidate: bool,
    last_contact: f64,
    tail: Option<(f64, f64)>,
}
impl ScrollGuard {
    pub fn contacts(&mut self, count: usize, eligible: bool, active: bool, now: f64) {
        self.last_contact = now;
        if eligible && (count == 3 || active) {
            self.candidate = true;
            self.tail = None;
        } else if count == 0 && self.candidate {
            self.candidate = false;
            self.tail = Some((now, now));
        } else if count != 0 || !eligible {
            self.candidate = false;
            self.tail = None;
        }
    }
    pub fn capture(&mut self, continuous: bool, momentum: i64, now: f64) -> bool {
        if !continuous {
            return false;
        }
        if self.candidate && now - self.last_contact < 0.5 {
            return true;
        }
        if let Some((lift, last)) = self.tail {
            // Keep draining a long owned fling until its end packet. The timeout
            // measures silence between packets, not the total fling duration.
            let elapsed = now - lift;
            let capture = (0. ..=2.).contains(&(now - last))
                && (momentum != 0 || (0. ..0.15).contains(&elapsed));
            if capture && momentum != 0 {
                self.tail = if momentum == 3 {
                    None
                } else {
                    Some((lift, now))
                };
            }
            capture
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelRoute {
    Pass,
    Consume,
    Navigate(f64),
}

#[derive(Default)]
pub struct WheelRouter {
    owned_until: Option<f64>,
}
impl WheelRouter {
    pub fn route(
        &mut self,
        held: bool,
        delta: f64,
        continuous: bool,
        momentum: i64,
        phase: i64,
        now: f64,
    ) -> WheelRoute {
        if continuous && phase & 1 != 0 {
            self.owned_until = None;
        }
        let owned = self.owned_until.is_some_and(|until| now <= until);
        if momentum != 0 {
            // We supply our own deceleration: never navigate twice using OS momentum.
            return if owned {
                self.owned_until = if momentum == 3 { None } else { Some(now + 2.) };
                WheelRoute::Consume
            } else {
                WheelRoute::Pass
            };
        }
        if !held {
            if continuous && owned {
                return WheelRoute::Consume;
            }
            // An unrelated physical mouse wheel must not release ownership of
            // a trackpad stream that is still delivering native momentum.
            return WheelRoute::Pass;
        }
        if !delta.is_finite() || delta == 0. {
            return if continuous && owned {
                WheelRoute::Consume
            } else {
                WheelRoute::Pass
            };
        }
        if continuous {
            self.owned_until = Some(now + 2.);
        }
        WheelRoute::Navigate(if continuous {
            -delta / 90.
        } else {
            // Accelerated line counts differ across mice; one detent is one entry.
            -delta.signum()
        })
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Mouse-wheel intent is deliberately more conservative than trackpad intent.
/// A short burst stays within one slot. Sustained traversal requires both a
/// longer burst and extra travel after the waiting period; old travel is not replayed.
#[derive(Default, Debug)]
pub struct WheelScroll {
    started: Option<f64>,
    raw: f64,
    anchor: f64,
    position: f64,
    sustained: bool,
}
impl WheelScroll {
    pub fn sample(&mut self, delta: f64, time: f64) -> f64 {
        if !delta.is_finite() || !time.is_finite() {
            return self.position;
        }
        let started = *self.started.get_or_insert(time);
        self.raw += delta;
        if !self.sustained {
            if time - started < 0.65 {
                self.anchor = self.raw;
            } else if (self.raw - self.anchor).abs() >= 2.5 {
                self.sustained = true;
            }
        }
        self.position = if self.sustained {
            self.position + delta * 0.45
        } else {
            (self.position + delta).clamp(-1., 1.)
        };
        self.position
    }
    pub fn sustained(&self) -> bool {
        self.sustained
    }
}

/// Velocity is measured from timestamped contact samples, not main-thread delivery
/// times. Releasing after a pause stops immediately; a flick decays exponentially.
#[derive(Default, Debug)]
pub struct KineticScroll {
    sample: Option<(f64, f64)>,
    velocity: f64,
    pub offset: i32,
    position: f64,
    travel: f64,
    coasting: bool,
    friction: f64,
    travel_limit: f64,
}
impl KineticScroll {
    pub fn sample(&mut self, position: f64, time: f64) {
        if !position.is_finite() || !time.is_finite() {
            self.velocity = 0.;
            self.coasting = false;
            return;
        }
        if let Some((previous, at)) = self.sample {
            let dt = time - at;
            if dt > 0. && dt <= 0.15 {
                let speed = ((position - previous) / dt).clamp(-35., 35.);
                if speed * self.velocity < 0. {
                    self.velocity = speed;
                } else {
                    let weight = 1. - (-dt / 0.04).exp();
                    self.velocity += (speed - self.velocity) * weight;
                }
            } else if dt > 0.15 {
                self.velocity = 0.;
            } else {
                return;
            }
        }
        self.sample = Some((position, time));
        self.coasting = false;
    }
    pub fn release(&mut self, time: f64, sustained: bool) -> bool {
        let fresh = self
            .sample
            .is_some_and(|(_, at)| (0. ..=0.13).contains(&(time - at)));
        // Speed alone never turns an ordinary swipe into a fling. Sustained
        // intent is supplied by contact traversal (or a long wheel burst).
        self.coasting = fresh && sustained && self.velocity.abs() >= 2.5;
        if self.coasting {
            let speed = (self.velocity.abs() / 35.).clamp(0., 1.);
            self.velocity *= 1. + 0.8 * speed;
            self.friction = 5.5 - 2.3 * speed;
            self.travel_limit = (self.velocity.abs() * 0.35).clamp(2., 18.);
        }
        self.position = self.offset as f64;
        self.travel = 0.;
        self.coasting
    }
    /// Mouse wheels get a short, restrained coast independent of the stronger
    /// sustained-trackpad profile: at most one additional selected entry.
    pub fn release_wheel(&mut self, time: f64, sustained: bool) -> bool {
        if self.release(time, sustained) {
            self.velocity = self.velocity.clamp(-6., 6.);
            self.friction = 8.;
            self.travel_limit = 1.;
        }
        self.coasting
    }
    pub fn step(&mut self, seconds: f64) -> Option<i32> {
        if !self.coasting || !seconds.is_finite() || seconds <= 0. {
            return None;
        }
        let dt = seconds.min(0.05);
        let decay = (-dt * self.friction).exp();
        let movement = self.velocity * (1. - decay) / self.friction;
        self.velocity *= decay;
        self.position += movement;
        self.travel += movement.abs();
        let before = self.offset;
        while self.position - self.offset as f64 > 0.65 {
            self.offset += 1;
        }
        while self.position - (self.offset as f64) < -0.65 {
            self.offset -= 1;
        }
        if self.velocity.abs() < 0.3 || self.travel >= self.travel_limit {
            self.coasting = false;
        }
        (self.offset != before).then_some(self.offset)
    }
    pub fn coasting(&self) -> bool {
        self.coasting
    }
    pub fn direction(&self) -> i32 {
        self.sample.map_or(0, |(p, _)| p.signum() as i32)
    }
}
