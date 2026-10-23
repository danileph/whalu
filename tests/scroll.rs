use magnetic::scroll::{KineticScroll, ScrollGuard, WheelRoute, WheelRouter, WheelScroll};

#[test]
fn three_contact_scroll_is_claimed_before_gesture_threshold_and_through_lift() {
    let mut guard = ScrollGuard::default();
    guard.contacts(3, true, false, 0.);
    assert!(guard.capture(true, 0, 0.01));
    assert!(!guard.capture(false, 0, 0.01)); // independent mouse wheel
    guard.contacts(3, true, true, 0.08);
    guard.contacts(2, true, true, 0.10); // partial release of this same gesture
    assert!(guard.capture(true, 0, 0.11));
    guard.contacts(0, true, false, 0.12);
    assert!(guard.capture(true, 0, 0.14));
    assert!(guard.capture(true, 1, 0.3));
    assert!(guard.capture(true, 2, 0.6));
    assert!(!guard.capture(true, 0, 0.6));
    assert!(!guard.capture(true, 2, 3.));
}

#[test]
fn normal_two_and_four_contact_streams_pass_and_new_touches_release_old_momentum() {
    for count in [1, 2, 4] {
        let mut guard = ScrollGuard::default();
        guard.contacts(count, true, false, 0.);
        assert!(!guard.capture(true, 0, 0.01));
        guard.contacts(3, true, true, 0.1);
        guard.contacts(0, true, false, 0.2);
        guard.contacts(count, true, false, 0.3);
        assert!(!guard.capture(true, 2, 0.31));
    }
}

#[test]
fn rejected_or_stale_contact_sequences_do_not_capture_scroll() {
    let mut guard = ScrollGuard::default();
    guard.contacts(3, false, false, 0.); // a third finger joined an established scroll
    assert!(!guard.capture(true, 0, 0.01));
    guard.contacts(3, true, false, 0.1);
    assert!(!guard.capture(true, 0, 1.));
    guard.contacts(4, false, false, 1.1); // cancelled navigation
    assert!(!guard.capture(true, 2, 1.12));
}

#[test]
fn modifier_wheel_has_normal_direction_and_consumes_only_its_momentum() {
    let mut router = WheelRouter::default();
    assert_eq!(router.route(false, -3., false, 0, 0, 0.), WheelRoute::Pass);
    assert_eq!(
        router.route(true, -3., false, 0, 0, 0.1),
        WheelRoute::Navigate(1.)
    );
    assert_eq!(
        router.route(true, 1., false, 0, 0, 0.2),
        WheelRoute::Navigate(-1.)
    );
    assert_eq!(
        router.route(true, -45., true, 0, 1, 0.3),
        WheelRoute::Navigate(0.5)
    );
    assert_eq!(
        router.route(false, -3., false, 0, 0, 0.35),
        WheelRoute::Pass
    );
    assert_eq!(
        router.route(false, -20., true, 2, 0, 0.4),
        WheelRoute::Consume
    );
    assert_eq!(
        router.route(false, -20., true, 0, 2, 0.4),
        WheelRoute::Consume
    );
    // A fresh normal two-finger scroll must not inherit ownership after modifier release.
    assert_eq!(router.route(false, -20., true, 0, 1, 0.5), WheelRoute::Pass);
    assert_eq!(router.route(false, -20., true, 2, 0, 0.6), WheelRoute::Pass);
    router.reset();
    // Pressing the modifier in an already coasting native page does not steal its fling.
    assert_eq!(router.route(true, -20., true, 2, 0, 0.7), WheelRoute::Pass);
}

fn flick(direction: f64) -> KineticScroll {
    let mut motion = KineticScroll::default();
    for (time, position) in [(0., 0.), (0.04, 0.4), (0.08, 0.9), (0.12, 1.5), (0.16, 2.)] {
        motion.sample(position * direction, time);
    }
    motion.offset = 2 * direction as i32;
    motion
}

#[test]
fn flick_coasts_in_the_same_direction_decelerates_and_terminates() {
    for direction in [-1., 1.] {
        let mut motion = flick(direction);
        assert!(motion.release(0.17, true));
        let mut changes = vec![];
        for frame in 1..=120 {
            if let Some(offset) = motion.step(1. / 60.) {
                changes.push((frame, offset));
            }
        }
        assert!(!changes.is_empty());
        assert!(
            changes
                .iter()
                .all(|(_, offset)| (*offset as f64) * direction > 2.)
        );
        assert!(motion.offset.abs() <= 20);
        assert!(!motion.coasting());
        assert_eq!(motion.step(1.), None);
    }
}

#[test]
fn holding_before_lift_and_short_swipes_do_not_launch_long_scrolls() {
    let mut held = flick(1.);
    held.sample(2., 0.3);
    held.sample(2., 0.5);
    assert!(!held.release(0.51, true));
    let mut stale = flick(1.);
    assert!(!stale.release(0.5, true));
    let mut short = KineticScroll::default();
    short.sample(0., 0.);
    short.sample(0.8, 0.04);
    assert!(!short.release(0.05, false));
}

#[test]
fn reversal_changes_momentum_direction_and_new_input_catches_a_coast() {
    let mut motion = flick(1.);
    motion.sample(1.6, 0.2);
    motion.sample(1.2, 0.24);
    motion.offset = 1;
    assert!(motion.release(0.25, true));
    let mut reverse = None;
    for _ in 0..60 {
        reverse = reverse.or(motion.step(1. / 60.));
    }
    assert!(reverse.is_some_and(|offset| offset <= 0));
    motion.sample(0., 0.3);
    assert!(!motion.coasting());
}

#[test]
fn long_owned_momentum_stays_captured_until_its_end_packet() {
    let mut guard = ScrollGuard::default();
    guard.contacts(3, true, true, 0.);
    guard.contacts(0, true, false, 0.1);
    for second in 1..=6 {
        assert!(guard.capture(true, 2, second as f64));
    }
    assert!(guard.capture(true, 3, 6.1));
    assert!(!guard.capture(true, 2, 6.2));
    let mut wheel = WheelRouter::default();
    wheel.route(true, -45., true, 0, 1, 0.);
    for second in 1..=6 {
        assert_eq!(
            wheel.route(false, -20., true, 2, 0, second as f64),
            WheelRoute::Consume
        );
    }
    assert_eq!(wheel.route(false, 0., true, 3, 0, 6.1), WheelRoute::Consume);
    assert_eq!(wheel.route(false, -20., true, 2, 0, 6.2), WheelRoute::Pass);
}

#[test]
fn ordinary_swipes_never_coast_even_when_fast_and_wide() {
    let mut motion = KineticScroll::default();
    motion.sample(0., 0.);
    motion.sample(1., 0.025);
    motion.offset = 1;
    assert!(!motion.release(0.03, false));
    for _ in 0..180 {
        assert_eq!(motion.step(1. / 60.), None);
    }
    assert_eq!(motion.offset, 1);
}

#[test]
fn sustained_flick_speed_controls_coast_distance_and_duration() {
    let coast = |speed: f64| {
        let mut motion = KineticScroll::default();
        for i in 0..=12 {
            motion.sample(i as f64 * 0.04 * speed, i as f64 * 0.04);
        }
        assert!(motion.release(0.49, true));
        let mut frames = 0;
        while motion.coasting() && frames < 240 {
            motion.step(1. / 60.);
            frames += 1;
        }
        assert!(!motion.coasting());
        (motion.offset, frames)
    };
    let slow = coast(4.);
    let fast = coast(25.);
    assert!(fast.0 > slow.0 + 4, "fast={fast:?}, slow={slow:?}");
    assert!(fast.1 > slow.1);
}

#[test]
fn real_contact_intent_controls_release_momentum() {
    use magnetic::gestures::{Contact, GestureEvent, SwipeRecognizer};
    let run = |path: &[(f64, f64)]| {
        let mut recognizer = SwipeRecognizer::default();
        let mut motion = KineticScroll::default();
        let mut sustained = false;
        for &(time, displacement) in path {
            let contacts: Vec<_> = (0..3)
                .map(|id| Contact {
                    id,
                    x: 0.3 + id as f64 * 0.08,
                    y: 0.35 + displacement,
                })
                .collect();
            for event in recognizer.feed(time, &contacts) {
                if let GestureEvent::Update(offset) = event {
                    motion.offset = offset;
                }
            }
            if let Some(position) = recognizer.progress() {
                motion.sample(position, time);
                sustained = recognizer.sustained();
            }
        }
        let release = path.last().unwrap().0 + 0.02;
        recognizer.feed(release, &[]);
        let offset = motion.offset;
        let coast = motion.release(release, sustained);
        for _ in 0..240 {
            motion.step(1. / 60.);
        }
        (offset, coast, motion.offset)
    };
    let ordinary = run(&[
        (0., 0.),
        (0.04, 0.1),
        (0.08, 0.2),
        (0.12, 0.3),
        (0.16, 0.4),
        (0.2, 0.5),
    ]);
    assert_eq!(ordinary, (1, false, 1));
    let long = run(&[
        (0., 0.),
        (0.04, 0.04),
        (0.08, 0.10),
        (0.2, 0.12),
        (0.3, 0.12),
        (0.34, 0.20),
        (0.38, 0.30),
        (0.42, 0.40),
    ]);
    assert!(long.0 > 1 && long.1 && long.2 > long.0 + 3, "{long:?}");
}

#[test]
fn wheel_traversal_starts_later_and_does_not_replay_early_travel() {
    let mut wheel = WheelScroll::default();
    for i in 0..=8 {
        assert_eq!(wheel.sample(1., i as f64 * 0.1), 1.);
        assert!(!wheel.sustained());
    }
    let unlocked = wheel.sample(1., 0.9);
    assert!(wheel.sustained());
    assert!(unlocked > 1. && unlocked < 1.5);
    assert_eq!(unlocked.round(), 1.);
    let next = wheel.sample(1., 1.);
    assert_eq!(next.round(), 2.);
    assert!((next - unlocked - 0.45).abs() < 0.001);
    // Short bursts stay at one slot and reverse without undoing earlier overshoot.
    let mut short = WheelScroll::default();
    for i in 0..20 {
        assert_eq!(short.sample(1., i as f64 * 0.01), 1.);
    }
    assert_eq!(short.sample(-1., 0.21), 0.);
    assert!(!short.sustained());
}

#[test]
fn mouse_wheel_coast_is_gentler_than_sustained_trackpad_coast() {
    let coast = |wheel: bool| {
        let mut motion = KineticScroll::default();
        for i in 0..=10 {
            motion.sample(i as f64, i as f64 * 0.04);
        }
        motion.offset = 10;
        if wheel {
            assert!(motion.release_wheel(0.51, true));
        } else {
            assert!(motion.release(0.51, true));
        }
        for _ in 0..240 {
            motion.step(1. / 60.);
        }
        motion.offset - 10
    };
    let wheel = coast(true);
    let trackpad = coast(false);
    assert!(wheel <= 1);
    assert!(trackpad > wheel + 4);
}
