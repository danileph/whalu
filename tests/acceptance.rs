use whalu::{
    config::Config,
    core::{Command, Effect, Engine, Mode, Window},
    geometry::{Direction, Rect, Region},
    input::{Modifiers, route},
    navigation::ClickSequence,
    persistence::SavedState,
};
fn window(id: u64, app: &str, frame: Rect) -> Window {
    Window {
        id,
        app_id: app.into(),
        app_name: app.into(),
        title: format!("Document {id}"),
        display: 1,
        frame,
        mode: Mode::Magnetic,
        last_floating_frame: None,
        snap: None,
        restore_frame: None,
    }
}
fn engine() -> Engine {
    let mut e = Engine::new(8.);
    e.bounds.insert(1, Rect::new(0., 30., 1440., 850.));
    for (id, app, x) in [
        (1, "Safari", 0.),
        (2, "Editor", 100.),
        (3, "Terminal", 760.),
        (4, "Finder", 250.),
    ] {
        e.windows
            .insert(id, window(id, app, Rect::new(x, 70., 650., 700.)));
    }
    e.set_visible(1, &[1, 2, 3, 4], Some(3));
    e
}
#[test]
fn stack_visits_each_individual_window() {
    let mut e = engine();
    assert_eq!(e.command(Command::FocusPrevious), vec![Effect::Focus(2)]);
    e.focus(4);
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(1)]);
}
#[test]
fn thumbnail_click_visits_intermediate_windows_in_both_directions_without_wrapping() {
    for (start, target, expected) in [
        (1, 4, vec![1, 2, 3, 4]),
        (4, 1, vec![4, 3, 2, 1]),
        (2, 4, vec![2, 3, 4]),
        (3, 1, vec![3, 2, 1]),
        (2, 3, vec![2, 3]),
        (3, 3, vec![3]),
    ] {
        let mut e = engine();
        e.focus(start);
        let mut click = ClickSequence::new(&e, target).unwrap();
        let mut visible = vec![start];
        e.focus(target);
        while let Some(window) = click.advance(&e) {
            visible.push(window);
            assert_eq!(e.focused(), Some(target));
        }
        assert_eq!(visible, expected);
        assert_eq!(e.context().unwrap().order, vec![1, 2, 3, 4]);
        assert!(!click.pending());
    }
}
#[test]
fn thumbnail_travel_stops_after_failed_focus_external_focus_space_change_or_removal() {
    for interruption in 0..5 {
        let mut e = engine();
        e.focus(1);
        let mut click = ClickSequence::new(&e, 4).unwrap();
        if interruption != 0 {
            e.focus(4);
            assert_eq!(click.advance(&e), Some(2));
        }
        match interruption {
            0 => {} // The requested focus did not succeed.
            1 => e.focus(1),
            2 => e.set_visible(2, &[1, 2, 3, 4], Some(4)),
            3 => {
                e.windows.remove(&3);
            }
            4 => e.set_visible(1, &[1, 2, 3], Some(2)),
            _ => unreachable!(),
        }
        assert_eq!(click.advance(&e), None);
        assert!(!click.pending());
    }
}
#[test]
fn thumbnail_retarget_starts_at_current_selection_and_handles_unfocused_contexts() {
    let mut e = engine();
    e.focus(1);
    let mut first = ClickSequence::new(&e, 4).unwrap();
    e.focus(4);
    assert_eq!(first.advance(&e), Some(2));
    assert_eq!(e.focused(), Some(4));
    let mut reversed = ClickSequence::from_selection(&e, 1, first.selected()).unwrap();
    e.focus(1);
    assert_eq!(reversed.advance(&e), Some(1));
    assert!(!reversed.pending());
    assert!(ClickSequence::new(&e, 99).is_none());
    e.set_visible(1, &[1, 2, 3, 4], None);
    let mut unfocused = ClickSequence::new(&e, 4).unwrap();
    e.focus(4);
    assert_eq!(unfocused.advance(&e), Some(4));
    assert!(!unfocused.pending());
}
#[test]
fn app_navigation_is_window_based() {
    let mut e = engine();
    e.windows.get_mut(&1).unwrap().app_id = "Terminal".into();
    assert_eq!(e.command(Command::AppNext), vec![Effect::Focus(1)]);
    e.focus(2);
    assert!(e.command(Command::AppNext).is_empty());
}
#[test]
fn floating_rectangles_are_invariant_under_automatic_layout() {
    let mut e = engine();
    e.focus(1);
    e.command(Command::ToggleMode);
    let before = e.windows[&1].frame;
    for active in [2, 3, 4, 1] {
        e.focus(active);
        for effect in e.plan() {
            match effect {
                Effect::Move(id, r) => {
                    assert_ne!(id, 1);
                    let w = &e.windows[&id];
                    assert_eq!((w.frame.width, w.frame.height), (r.width, r.height));
                }
                _ => panic!("Automatic layout emitted a non-move"),
            }
        }
    }
    assert_eq!(e.windows[&1].frame, before);
    assert_eq!(e.windows[&1].last_floating_frame, Some(before));
}
#[test]
fn automatic_layout_preserves_every_size_for_many_geometries() {
    for n in 0..160 {
        let mut e = engine();
        e.focus(1);
        e.windows.get_mut(&4).unwrap().frame = Rect::new(
            (n * 37 % 1500) as f64 - 200.,
            (n * 23 % 1000) as f64 - 100.,
            (n * 19 % 1400 + 100) as f64,
            (n * 29 % 900 + 80) as f64,
        );
        for effect in e.plan() {
            let Effect::Move(id, r) = effect else {
                panic!()
            };
            let w = &e.windows[&id];
            assert_eq!(w.mode, Mode::Magnetic);
            assert_eq!(w.frame.width, r.width);
            assert_eq!(w.frame.height, r.height);
            assert!(r.valid());
        }
    }
}
#[test]
fn direct_snap_can_resize_floating_window_and_restore() {
    let mut e = engine();
    e.focus(1);
    e.command(Command::ToggleMode);
    let before = e.windows[&1].frame;
    assert_eq!(
        e.command(Command::Snap(Direction::Up)),
        vec![Effect::Snap(1, Region::Full.rect(e.bounds[&1]))]
    );
    assert_eq!(
        e.command(Command::Snap(Direction::Down)),
        vec![Effect::Snap(1, before)]
    );
    assert_eq!(e.windows[&1].mode, Mode::Floating);
}
#[test]
fn frozen_engine_never_moves_or_focuses() {
    let mut e = engine();
    e.frozen = true;
    assert!(e.plan().is_empty());
    assert!(e.command(Command::FocusNext).is_empty());
    assert!(e.command(Command::Snap(Direction::Up)).is_empty());
}
#[test]
fn native_contexts_keep_independent_navigation_and_modes() {
    let mut e = engine();
    let original = e.context().unwrap().clone();
    e.windows.get_mut(&3).unwrap().mode = Mode::Floating;
    e.set_visible(2, &[4], Some(4));
    assert!(e.command(Command::FocusNext).is_empty());
    e.set_visible(1, &[1, 2, 3], Some(3));
    assert_eq!(e.context().unwrap().order, original.order[..3]);
    assert_eq!(e.windows[&3].mode, Mode::Floating);
}
#[test]
fn snap_uses_correct_display_with_negative_origin() {
    let mut e = engine();
    let b = Rect::new(-1920., -300., 1920., 1080.);
    e.bounds.insert(2, b);
    e.windows.get_mut(&3).unwrap().display = 2;
    let out = e.command(Command::Snap(Direction::Right));
    assert_eq!(out, vec![Effect::Snap(3, Region::Right.rect(b))]);
    for r in [
        Region::Full,
        Region::Left,
        Region::Right,
        Region::TopLeft,
        Region::BottomLeft,
        Region::TopRight,
        Region::BottomRight,
    ] {
        let f = r.rect(b);
        assert!(f.valid());
        assert_eq!(f.intersection(b), f);
    }
}
#[test]
fn default_shortcuts_respect_native_and_reserved_keys() {
    let c = Config::default();
    c.validate().unwrap();
    for m in [
        Modifiers {
            control: true,
            ..Default::default()
        },
        Modifiers {
            command: true,
            ..Default::default()
        },
        Modifiers {
            option: true,
            ..Default::default()
        },
    ] {
        for key in [123, 124, 125, 126, 48, 46] {
            assert_eq!(route(&c.bindings, key, m), None);
        }
    }
    let m = Modifiers {
        control: true,
        option: true,
        ..Default::default()
    };
    assert_eq!(route(&c.bindings, 125, m), Some(Command::FocusNext));
    assert_eq!(
        route(&c.bindings, 126, Modifiers { shift: true, ..m }),
        None
    );
    assert!(m.overlay(c.overlay_modifiers));
    assert!(!Modifiers { option: false, ..m }.overlay(c.overlay_modifiers));
}
#[test]
fn config_round_trip_and_validation() {
    let c = Config::default();
    let t = toml::to_string(&c).unwrap();
    let mut d: Config = toml::from_str(&t).unwrap();
    d.validate().unwrap();
    d.bindings.push(d.bindings[0].clone());
    assert!(d.validate().is_err());
    d = Config::default();
    d.gap = f64::NAN;
    assert!(d.validate().is_err());
    assert!(toml::from_str::<Config>("unexpected = true").is_err());
    let old_config: Config = toml::from_str("overlay = true\noverlay_brightness = 0.1").unwrap();
    old_config.validate().unwrap();
    assert!(old_config.overlay);
    assert!(
        !toml::to_string(&old_config)
            .unwrap()
            .contains("overlay_brightness")
    );
}
#[test]
fn recovery_restores_unique_modes_without_moving_windows() {
    let mut e = engine();
    e.windows.get_mut(&3).unwrap().mode = Mode::Floating;
    let saved = SavedState::capture(&e);
    let mut next = engine();
    let rects: Vec<_> = next.windows.values().map(|w| w.frame).collect();
    saved.restore_modes(&mut next.windows);
    assert_eq!(next.windows[&3].mode, Mode::Floating);
    assert_eq!(
        rects,
        next.windows.values().map(|w| w.frame).collect::<Vec<_>>()
    );
}
#[test]
fn ambiguous_recovery_never_assigns_a_saved_mode() {
    let mut e = engine();
    e.windows.get_mut(&3).unwrap().mode = Mode::Floating;
    let saved = SavedState::capture(&e);
    e.windows.get_mut(&3).unwrap().mode = Mode::Magnetic;
    let mut duplicate = e.windows[&3].clone();
    duplicate.id = 9;
    e.windows.insert(9, duplicate);
    saved.restore_modes(&mut e.windows);
    assert_eq!(e.windows[&3].mode, Mode::Magnetic);
    assert_eq!(e.windows[&9].mode, Mode::Magnetic);
}
#[test]
fn destroyed_windows_do_not_remain_in_navigation() {
    let mut e = engine();
    e.set_visible(1, &[1, 2, 4], Some(2));
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(4)]);
}

#[test]
fn navigation_can_enter_a_space_without_any_focused_window() {
    let mut e = engine();
    e.set_visible(2, &[1, 4], None);
    assert_eq!(e.focused(), None);
    for command in [Command::FocusNext, Command::AppNext] {
        assert_eq!(e.command(command), vec![Effect::Focus(1)]);
    }
    for command in [Command::FocusPrevious, Command::AppPrevious] {
        assert_eq!(e.command(command), vec![Effect::Focus(4)]);
    }
    assert!(e.command(Command::Snap(Direction::Up)).is_empty());
    assert!(e.command(Command::ToggleMode).is_empty());
    assert_eq!(e.focused(), None, "An anchor is not a real focused window");
}
#[test]
fn unfocused_space_resumes_its_last_focused_window() {
    let mut e = engine();
    e.focus(3);
    e.set_visible(2, &[4], Some(4));
    e.set_visible(1, &[1, 2, 3], None);
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(3)]);
    e.set_visible(1, &[1, 2], None);
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(1)]);
}
#[test]
fn context_tracking_does_not_forget_temporarily_invisible_members() {
    let mut tracker = whalu::visibility::ContextTracker::default();
    let a = tracker.resolve(&[1, 2, 3], false);
    assert_eq!(tracker.resolve(&[1], false), a);
    let b = tracker.resolve(&[4, 5], true);
    assert_ne!(a, b);
    assert_eq!(tracker.resolve(&[], true), 0);
    assert_eq!(tracker.resolve(&[2, 3], true), a);
    assert_eq!(tracker.resolve(&[1, 2, 3], false), a);
}
#[test]
fn snaps_are_exact_fractions_without_added_spacing() {
    for bounds in [
        Rect::new(0., 29., 1280., 803.),
        Rect::new(-1513., -200., 1513., 901.),
    ] {
        let left = Region::Left.rect(bounds);
        let right = Region::Right.rect(bounds);
        assert_eq!(left.width, bounds.width / 2.);
        assert_eq!(right.width, bounds.width / 2.);
        assert_eq!(left.x, bounds.x);
        assert_eq!(left.right(), right.x);
        assert_eq!(right.right(), bounds.right());
        assert_eq!(left.height, bounds.height);
        assert_eq!(right.height, bounds.height);
        assert_eq!(Region::Full.rect(bounds), bounds);
        let tl = Region::TopLeft.rect(bounds);
        let bl = Region::BottomLeft.rect(bounds);
        let tr = Region::TopRight.rect(bounds);
        let br = Region::BottomRight.rect(bounds);
        assert_eq!(tl.right(), tr.x);
        assert_eq!(tl.bottom(), bl.y);
        assert_eq!(bl.right(), br.x);
        assert_eq!(tr.bottom(), br.y);
        assert_eq!(
            [tl, tr, bl, br].iter().map(|r| r.area()).sum::<f64>(),
            bounds.area()
        );
        // Automatic-layout spacing must not shrink explicit snaps.
        for gap in [0., 8., 100.] {
            let mut e = engine();
            e.gap = gap;
            e.bounds.insert(1, bounds);
            e.focus(1);
            assert_eq!(
                e.command(Command::Snap(Direction::Left)),
                vec![Effect::Snap(1, left)]
            );
        }
    }
}

#[test]
fn minimum_size_snaps_preserve_outer_edges_and_keep_the_actual_window_visible() {
    use whalu::geometry::constrained_snap;
    for bounds in [
        Rect::new(0., 29., 1280., 803.),
        Rect::new(-1920., -300., 1920., 1080.),
    ] {
        for region in [
            Region::Left,
            Region::Right,
            Region::TopLeft,
            Region::TopRight,
            Region::BottomLeft,
            Region::BottomRight,
        ] {
            let target = region.rect(bounds);
            let actual = Rect::new(target.x, target.y, bounds.width * 0.7, bounds.height * 0.8);
            let aligned = constrained_snap(target, actual, bounds);
            assert_eq!(aligned.intersection(bounds), aligned);
            assert_eq!(aligned.width, actual.width);
            assert_eq!(aligned.height, actual.height);
            if matches!(
                region,
                Region::Right | Region::TopRight | Region::BottomRight
            ) {
                assert_eq!(aligned.right(), bounds.right());
            } else {
                assert_eq!(aligned.x, bounds.x);
            }
            if matches!(region, Region::BottomLeft | Region::BottomRight) {
                assert_eq!(aligned.bottom(), bounds.bottom());
            } else {
                assert_eq!(aligned.y, bounds.y);
            }
            assert_eq!(constrained_snap(target, target, bounds), target);
        }
        let oversized = Rect::new(0., 0., bounds.width + 100., bounds.height + 100.);
        let aligned = constrained_snap(Region::BottomRight.rect(bounds), oversized, bounds);
        assert_eq!((aligned.x, aligned.y), (bounds.x, bounds.y));
        assert_eq!(
            (aligned.width, aligned.height),
            (oversized.width, oversized.height)
        );
    }
}

#[test]
fn legacy_saved_pairings_are_ignored_but_modes_are_recovered() {
    let legacy = r#"{"version":1,"windows":[{"app_id":"Editor","title":"Document 2","mode":"floating","group":42},{"app_id":"Terminal","title":"Document 3","mode":"magnetic","group":42}]}"#;
    let saved: SavedState = serde_json::from_str(legacy).unwrap();
    let mut e = engine();
    let before = e.windows[&2].frame;
    saved.restore_modes(&mut e.windows);
    assert_eq!(e.windows[&2].mode, Mode::Floating);
    assert_eq!(e.windows[&2].frame, before);
    e.focus(2);
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(3)]);
    assert!(
        !serde_json::to_string(&SavedState::capture(&e))
            .unwrap()
            .contains("group")
    );
}
#[test]
fn snapped_windows_remain_independent_and_hold_their_explicit_rectangles() {
    let mut e = engine();
    for (id, dir) in [(1, Direction::Left), (4, Direction::Right)] {
        e.focus(id);
        let effects = e.command(Command::Snap(dir));
        let [Effect::Snap(_, target)] = effects.as_slice() else {
            panic!()
        };
        let target = *target;
        e.observed_frame(id, target, false);
    }
    e.focus(1);
    assert_eq!(e.command(Command::FocusNext), vec![Effect::Focus(2)]);
    for active in [1, 2, 3, 4] {
        e.focus(active);
        assert!(
            e.plan()
                .iter()
                .all(|effect| !matches!(effect, Effect::Move(1 | 4, _)))
        );
    }
}
#[test]
fn former_group_and_spatial_shortcuts_are_not_captured() {
    let c = Config::default();
    let base = Modifiers {
        control: true,
        option: true,
        ..Default::default()
    };
    for key in [125, 126] {
        assert_eq!(
            route(
                &c.bindings,
                key,
                Modifiers {
                    shift: true,
                    ..base
                }
            ),
            None
        );
    }
    for key in [4, 38, 40, 37] {
        assert_eq!(route(&c.bindings, key, base), None);
    }
}
