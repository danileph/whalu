use whalu::{
    core::{Command, Effect, Engine, Mode, Window},
    geometry::Rect,
    gestures::{Axis, Contact, GestureEvent as Event, SwipeRecognizer},
    navigation::ScrubSession,
};
fn contacts(n: usize, dx: f64, dy: f64) -> Vec<Contact> {
    (0..n)
        .map(|i| Contact {
            id: i as i32,
            x: 0.3 + i as f64 * 0.08 + dx,
            y: 0.35 + dy,
        })
        .collect()
}
fn trace(n: usize, dx: f64, dy: f64) -> Vec<Event> {
    let mut r = SwipeRecognizer::new(0.12);
    let mut events = vec![];
    for i in 0..6 {
        events.extend(r.feed(
            i as f64 * 0.04,
            &contacts(n, dx * i as f64 / 5., dy * i as f64 / 5.),
        ));
    }
    events.extend(r.feed(0.26, &[]));
    events
}
#[test]
fn directions_start_and_update_before_release() {
    for (dx, dy, axis, sign) in [
        (0., 0.25, Axis::Vertical, 1),
        (0., -0.25, Axis::Vertical, -1),
    ] {
        let events = trace(3, dx, dy);
        assert_eq!(events.first(), Some(&Event::Begin(axis)));
        assert!(
            events
                .iter()
                .any(|e| matches!(e,Event::Update(i) if i.signum()==sign&&i.abs()==1))
        );
        assert_eq!(events.last(), Some(&Event::End { cancelled: false }));
    }
}
#[test]
fn continuous_scrub_can_move_several_entries_and_reverse_without_lifting() {
    let mut r = SwipeRecognizer::new(0.12);
    let mut events = vec![];
    for (t, x) in [
        (0., 0.),
        (0.04, 0.03),
        (0.08, 0.08),
        (0.2, 0.12),
        (0.3, 0.16),
        (0.34, 0.24),
        (0.4, 0.36),
        (0.46, 0.44),
        (0.52, 0.28),
        (0.58, 0.12),
        (0.64, -0.16),
    ] {
        events.extend(r.feed(t, &contacts(3, 0., x)));
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Begin(_)))
            .count(),
        1
    );
    assert!(r.sustained());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Update(i) if *i >= 3))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Update(i) if *i <= -1))
    );
    assert!(!events.iter().any(|e| matches!(e, Event::End { .. })));
    assert_eq!(r.feed(0.7, &[]), vec![Event::End { cancelled: false }]);
    assert!(r.feed(0.8, &[]).is_empty());
}
#[test]
fn holding_fingers_keeps_session_open_and_jitter_does_not_repeat() {
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    r.feed(0.04, &contacts(3, 0., 0.04));
    r.feed(0.08, &contacts(3, 0., 0.1));
    for t in [1., 3., 20.] {
        assert!(r.feed(t, &contacts(3, 0., 0.11)).is_empty());
    }
    assert_eq!(r.feed(20.1, &contacts(3, 0., 0.24)), vec![Event::Update(4)]);
    assert!(r.sustained());
    assert_eq!(r.feed(20.2, &[]), vec![Event::End { cancelled: false }]);
}
#[test]
fn two_and_four_finger_gestures_never_open_a_session() {
    for n in [1, 2, 4, 5] {
        assert!(trace(n, 0.25, 0.).is_empty());
        assert!(trace(n, 0., 0.25).is_empty());
    }
}
#[test]
fn normal_four_finger_landing_cancels_before_overview_appears() {
    let mut r = SwipeRecognizer::default();
    assert!(r.feed(0., &contacts(3, 0., 0.)).is_empty());
    assert!(r.feed(0.02, &contacts(3, 0., 0.1)).is_empty());
    assert!(r.feed(0.04, &contacts(4, 0.2, 0.)).is_empty());
    assert!(r.feed(0.1, &[]).is_empty());
}
#[test]
fn a_fourth_finger_cancels_an_existing_session_once() {
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    r.feed(0.04, &contacts(3, 0., 0.04));
    assert!(
        r.feed(0.08, &contacts(3, 0., 0.15))
            .contains(&Event::Begin(Axis::Vertical))
    );
    assert_eq!(
        r.feed(0.12, &contacts(4, 0.2, 0.)),
        vec![Event::End { cancelled: true }]
    );
    assert!(r.feed(0.2, &contacts(3, 0., 0.2)).is_empty());
    assert!(r.feed(0.3, &[]).is_empty());
}
#[test]
fn partial_release_waits_for_all_fingers_to_lift() {
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    r.feed(0.04, &contacts(3, 0., 0.04));
    r.feed(0.08, &contacts(3, 0., 0.1));
    assert!(r.feed(0.1, &contacts(2, 0.1, 0.)).is_empty());
    assert!(r.feed(0.12, &contacts(1, 0.1, 0.)).is_empty());
    assert_eq!(r.feed(0.14, &[]), vec![Event::End { cancelled: false }]);
}
#[test]
fn pinches_diagonals_and_scrolls_do_not_open_sessions() {
    assert!(trace(3, 0.02, 0.02).is_empty());
    assert!(trace(3, 0.2, 0.2).is_empty());
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(2, 0., 0.));
    for i in 0..5 {
        assert!(
            r.feed(0.5 + i as f64 * 0.04, &contacts(3, i as f64 * 0.04, 0.))
                .is_empty()
        );
    }
    assert!(r.feed(0.8, &[]).is_empty());
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    for i in 1..5 {
        let mut p = contacts(3, 0., 0.);
        p[0].x -= 0.03 * i as f64;
        p[2].x += 0.03 * i as f64;
        assert!(r.feed(i as f64 * 0.04, &p).is_empty());
    }
}
#[test]
fn invalid_data_cancels_active_overview() {
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    r.feed(0.04, &contacts(3, 0., 0.04));
    r.feed(0.08, &contacts(3, 0., 0.1));
    let mut bad = contacts(3, 0.2, 0.);
    bad[0].x = f64::NAN;
    assert_eq!(r.feed(0.12, &bad), vec![Event::End { cancelled: true }]);
    assert!(r.feed(0.2, &[]).is_empty());
}
fn engine() -> Engine {
    let mut e = Engine::new(8.);
    for id in 1..=5 {
        e.windows.insert(
            id,
            Window {
                id,
                app_id: if id <= 3 {
                    "Editor".into()
                } else {
                    "Other".into()
                },
                app_name: "App".into(),
                title: format!("{id}"),
                display: 1,
                frame: Rect::new(id as f64 * 100., 0., 400., 300.),
                mode: Mode::Magnetic,
                last_floating_frame: None,
            },
        );
    }
    e.set_visible(1, &[1, 2, 3, 4, 5], Some(1));
    e.focus(3);
    e.focus(1);
    e
}
#[test]
fn vertical_scrub_visits_every_window_and_reverses() {
    let e = engine();
    let mut s = ScrubSession::new(&e, Axis::Vertical).unwrap();
    assert_eq!(s.update(1), Some(2));
    assert_eq!(s.update(2), Some(3));
    assert_eq!(s.update(3), Some(4));
    assert_eq!(s.update(4), Some(5));
    assert_eq!(s.update(3), Some(4));
    assert_eq!(s.update(0), Some(1));
    assert_eq!(s.original, Some(1));
}
#[test]
fn scrub_clamps_at_edges_and_reverses_immediately_after_overshoot() {
    let e = engine();
    let mut s = ScrubSession::new(&e, Axis::Vertical).unwrap();
    assert_eq!(s.update(20), Some(5));
    assert_eq!(s.update(21), None);
    assert_eq!(s.update(20), Some(4));
    assert_eq!(s.update(-20), Some(1));
    assert_eq!(s.update(-21), None);
    assert_eq!(s.update(-20), Some(2));
}
#[test]
fn unfocused_space_is_entered_without_skipping_first_entry() {
    let mut e = engine();
    e.set_visible(2, &[1, 2, 3, 4, 5], None);
    let mut s = ScrubSession::new(&e, Axis::Vertical).unwrap();
    assert_eq!(s.update(1), Some(1));
    assert_eq!(s.update(2), Some(2));
}
#[test]
fn live_focus_does_not_reorder_scrub_targets_or_change_rectangles() {
    let mut e = engine();
    let before: Vec<_> = e.windows.values().map(|w| w.frame).collect();
    let mut s = ScrubSession::new(&e, Axis::Vertical).unwrap();
    for offset in [1, 2, 3, 2, 1, 0] {
        if let Some(id) = s.update(offset) {
            e.focus(id);
        }
    }
    assert_eq!(
        e.windows.values().map(|w| w.frame).collect::<Vec<_>>(),
        before
    );
    assert_eq!(e.focused(), Some(1));
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(2)]);
}

#[test]
fn edge_hold_traverses_a_long_list_without_lifting_and_stops_when_reversed() {
    let mut r = SwipeRecognizer::default();
    r.feed(0., &contacts(3, 0., 0.));
    r.feed(0.04, &contacts(3, 0., 0.1));
    r.feed(0.08, &contacts(3, 0., 0.2));
    r.feed(0.2, &contacts(3, 0., 0.24));
    r.feed(0.3, &contacts(3, 0., 0.30));
    r.feed(0.34, &contacts(3, 0., 0.45));
    r.feed(0.4, &contacts(3, 0., 0.56));
    assert!(r.sustained());
    let a = r.feed(0.75, &contacts(3, 0., 0.56));
    let b = r.feed(0.95, &contacts(3, 0., 0.56));
    assert!(a.iter().any(|e| matches!(e,Event::Update(i) if *i>=6)));
    assert!(b.iter().any(|e| matches!(e,Event::Update(i) if *i>=7)));
    r.feed(1., &contacts(3, 0., 0.54));
    assert!(r.feed(1.5, &contacts(3, 0., 0.54)).is_empty());
    assert_eq!(r.feed(1.6, &[]), vec![Event::End { cancelled: false }]);
}

#[test]
fn ordinary_swipes_move_one_slot_at_any_speed_or_distance() {
    for distance in [0.10, 0.25, 0.55, -0.10, -0.25, -0.30] {
        let mut recognizer = SwipeRecognizer::default();
        let mut changes = vec![];
        for i in 0..=6 {
            changes.extend(
                recognizer.feed(i as f64 * 0.04, &contacts(3, 0., distance * i as f64 / 6.)),
            );
        }
        assert!(!recognizer.sustained());
        let updates: Vec<_> = changes
            .iter()
            .filter_map(|event| {
                if let Event::Update(offset) = event {
                    Some(*offset)
                } else {
                    None
                }
            })
            .collect();
        assert!(!updates.is_empty());
        assert!(
            updates
                .iter()
                .all(|offset| *offset == distance.signum() as i32)
        );
        // A long stationary hold, even at the edge, never arms acceleration.
        for time in [0.5, 1., 2., 3.] {
            assert!(recognizer.feed(time, &contacts(3, 0., distance)).is_empty());
            assert!(!recognizer.sustained());
        }
    }
}

#[test]
fn acceleration_requires_continued_motion_and_scales_with_speed() {
    let travel = |interval: f64| {
        let mut recognizer = SwipeRecognizer::default();
        for (time, position) in [
            (0., 0.),
            (0.04, 0.04),
            (0.08, 0.10),
            (0.2, 0.12),
            (0.3, 0.12),
        ] {
            recognizer.feed(time, &contacts(3, 0., position));
        }
        assert!(!recognizer.sustained());
        for i in 1..=4 {
            recognizer.feed(
                0.3 + interval * i as f64,
                &contacts(3, 0., 0.12 + i as f64 * 0.06),
            );
        }
        assert!(recognizer.sustained());
        recognizer.progress().unwrap()
    };
    let slow = travel(0.12);
    let fast = travel(0.025);
    assert!(slow > 1.);
    assert!(fast > slow * 1.5, "fast={fast}, slow={slow}");
}

#[test]
fn rapid_separate_swipes_each_move_one_slot_without_carrying_acceleration() {
    let mut recognizer = SwipeRecognizer::default();
    for stroke in 0..8 {
        let start = stroke as f64 * 0.3;
        let mut updates = vec![];
        for i in 0..=4 {
            for event in recognizer.feed(start + i as f64 * 0.04, &contacts(3, 0., i as f64 * 0.09))
            {
                if let Event::Update(offset) = event {
                    updates.push(offset);
                }
            }
            assert!(!recognizer.sustained());
        }
        assert_eq!(updates, vec![1]);
        assert_eq!(
            recognizer.feed(start + 0.18, &[]),
            vec![Event::End { cancelled: false }]
        );
    }
}

#[test]
fn four_rapid_short_strokes_select_at_recognition_before_each_lift() {
    let mut recognizer = SwipeRecognizer::default();
    for stroke in 0..4 {
        let t = stroke as f64 * 0.12;
        assert!(recognizer.feed(t, &contacts(3, 0., 0.)).is_empty());
        assert!(recognizer.feed(t + 0.02, &contacts(3, 0., 0.02)).is_empty());
        assert_eq!(
            recognizer.feed(t + 0.05, &contacts(3, 0., 0.04)),
            vec![Event::Begin(Axis::Vertical), Event::Update(1)]
        );
        // Slight recoil below recognition travel must not undo/repeat a slot.
        assert!(
            recognizer
                .feed(t + 0.065, &contacts(3, 0., 0.033))
                .is_empty()
        );
        assert!(recognizer.feed(t + 0.08, &contacts(3, 0., 0.07)).is_empty());
        assert!(!recognizer.sustained());
        assert_eq!(
            recognizer.feed(t + 0.09, &[]),
            vec![Event::End { cancelled: false }]
        );
    }
}

#[test]
fn short_stroke_selects_both_directions_without_waiting_for_scrub_distance() {
    for direction in [-1., 1.] {
        let mut recognizer = SwipeRecognizer::default();
        recognizer.feed(0., &contacts(3, 0., 0.));
        recognizer.feed(0.02, &contacts(3, 0., direction * 0.02));
        assert_eq!(
            recognizer.feed(0.05, &contacts(3, 0., direction * 0.04)),
            vec![
                Event::Begin(Axis::Vertical),
                Event::Update(direction as i32)
            ]
        );
        assert!(
            recognizer
                .feed(0.2, &contacts(3, 0., direction * 0.04))
                .is_empty()
        );
        assert!(
            recognizer
                .feed(1., &contacts(3, 0., direction * 0.04))
                .is_empty()
        );
        assert!(!recognizer.sustained());
    }
}

#[test]
fn stationary_three_contacts_prepare_overview_before_directional_navigation() {
    let mut recognizer = SwipeRecognizer::default();
    for time in [0., 0.02] {
        assert!(recognizer.feed(time, &contacts(3, 0., 0.)).is_empty());
        assert!(!recognizer.overview_ready());
    }
    for time in [0.05, 0.3, 1., 3.] {
        assert!(recognizer.feed(time, &contacts(3, 0., 0.)).is_empty());
        assert!(recognizer.overview_ready());
        assert!(!recognizer.active());
    }
    assert_eq!(
        recognizer.feed(3.05, &contacts(3, 0., 0.04)),
        vec![Event::Begin(Axis::Vertical), Event::Update(1)]
    );
    recognizer.feed(3.1, &[]);
    assert!(!recognizer.overview_ready());
}

#[test]
fn fourth_contact_and_late_third_contact_cannot_prepare_overview() {
    let mut recognizer = SwipeRecognizer::default();
    recognizer.feed(0., &contacts(3, 0., 0.));
    recognizer.feed(0.02, &contacts(3, 0., 0.));
    recognizer.feed(0.04, &contacts(4, 0., 0.));
    assert!(!recognizer.overview_ready());
    recognizer.feed(0.1, &contacts(3, 0., 0.));
    assert!(!recognizer.overview_ready());
    recognizer.feed(0.2, &[]);
    recognizer.feed(1., &contacts(2, 0., 0.));
    recognizer.feed(1.3, &contacts(3, 0., 0.));
    assert!(!recognizer.overview_ready());
}

#[test]
fn horizontal_strokes_and_holds_never_navigate_or_coast() {
    for direction in [-1., 1.] {
        let mut recognizer = SwipeRecognizer::default();
        // Include vertical drift below the directional threshold, sustained
        // movement, edge holds, reversal and partial release.
        for (time, x) in [
            (0., 0.),
            (0.04, 0.03),
            (0.08, 0.1),
            (0.4, 0.2),
            (0.5, 0.3),
            (1., 0.3),
            (3., 0.3),
            (3.1, -0.15),
        ] {
            assert!(
                recognizer
                    .feed(time, &contacts(3, direction * x, 0.02 * x))
                    .is_empty()
            );
            assert!(!recognizer.active());
            assert!(!recognizer.sustained());
            assert_eq!(recognizer.progress(), None);
        }
        assert!(recognizer.feed(3.2, &contacts(2, 0., 0.)).is_empty());
        assert!(recognizer.feed(3.3, &[]).is_empty());
        // The next vertical stroke still works normally.
        recognizer.feed(4., &contacts(3, 0., 0.));
        recognizer.feed(4.02, &contacts(3, 0., 0.02));
        assert_eq!(
            recognizer.feed(4.05, &contacts(3, 0., 0.04)),
            vec![Event::Begin(Axis::Vertical), Event::Update(1)]
        );
    }
}
