use whalu::{
    config::Config,
    core::{Command, Effect, Engine, Mode, Window},
    geometry::Rect,
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
fn drag_reorders_without_focus_or_geometry_changes_and_survives_discovery() {
    let mut e = engine();
    let frames: Vec<_> = e.windows.values().map(|w| w.frame).collect();
    assert!(e.reorder(1, 3));
    assert_eq!(e.context().unwrap().order, [2, 3, 4, 1]);
    assert_eq!(e.focused(), Some(3));
    e.set_visible(1, &[1, 2, 3, 4], Some(3));
    assert_eq!(e.context().unwrap().order, [2, 3, 4, 1]);
    assert!(e.reorder(1, 0));
    assert_eq!(e.context().unwrap().order, [1, 2, 3, 4]);
    assert_eq!(
        e.windows.values().map(|w| w.frame).collect::<Vec<_>>(),
        frames
    );
}
#[test]
fn drag_preserves_hidden_history_and_other_contexts() {
    let mut e = engine();
    e.set_visible(2, &[1, 2, 3, 4], Some(4));
    e.set_visible(1, &[1, 3, 4], Some(3));
    assert!(e.reorder(4, 0));
    assert_eq!(e.context().unwrap().order, [4, 1, 3]);
    assert_eq!(e.context().unwrap().history, [4, 2, 1, 3]);
    assert_eq!(e.contexts[&2].order, [1, 2, 3, 4]);
    e.set_visible(1, &[1, 2, 3, 4], Some(3));
    assert_eq!(e.context().unwrap().order, [4, 2, 1, 3]);
}
#[test]
fn stale_invalid_and_frozen_drops_do_nothing() {
    let mut e = engine();
    assert!(!e.reorder(99, 0));
    assert!(!e.reorder(1, 4));
    assert!(!e.reorder(1, 0));
    e.frozen = true;
    assert!(!e.reorder(1, 3));
    assert_eq!(e.context().unwrap().order, [1, 2, 3, 4]);
    let mut empty = Engine::new(8.);
    assert!(!empty.reorder(1, 0));
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
        while let Some(window) = click.advance(&e) {
            assert_eq!(e.focused(), visible.last().copied());
            e.focus(window);
            visible.push(window);
        }
        assert_eq!(e.focused(), Some(target));
        assert_eq!(visible, expected);
        assert_eq!(e.context().unwrap().order, vec![1, 2, 3, 4]);
        assert!(!click.pending());
    }
}
#[test]
fn thumbnail_travel_stops_after_failed_step_external_focus_space_change_or_removal() {
    for interruption in 0..5 {
        let mut e = engine();
        e.focus(1);
        let mut click = ClickSequence::new(&e, 4).unwrap();
        assert_eq!(click.advance(&e), Some(2));
        if interruption != 0 {
            e.focus(2);
        }
        match interruption {
            0 => {} // The requested first step did not succeed.
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
    assert_eq!(first.advance(&e), Some(2));
    e.focus(2);
    assert_eq!(e.focused(), Some(2));
    let mut reversed = ClickSequence::from_selection(&e, 1, first.selected()).unwrap();
    assert_eq!(reversed.advance(&e), Some(1));
    e.focus(1);
    assert!(!reversed.pending());
    assert!(ClickSequence::new(&e, 99).is_none());
    e.set_visible(1, &[1, 2, 3, 4], None);
    let mut unfocused = ClickSequence::new(&e, 4).unwrap();
    assert_eq!(unfocused.advance(&e), Some(1));
    e.focus(1);
    assert_eq!(unfocused.advance(&e), Some(2));
    e.focus(2);
    assert_eq!(unfocused.advance(&e), Some(3));
    e.focus(3);
    assert_eq!(unfocused.advance(&e), Some(4));
    e.focus(4);
    assert!(!unfocused.pending());
}
#[test]
fn first_thumbnail_click_after_space_reentry_traverses_from_its_local_anchor() {
    for (remembered, target, expected) in [
        (None, 4, vec![1, 2, 3, 4]),
        (Some(1), 4, vec![1, 2, 3, 4]),
        (Some(3), 4, vec![3, 4]),
        (Some(4), 1, vec![4, 3, 2, 1]),
        (Some(2), 2, vec![2]),
        (Some(99), 4, vec![1, 2, 3, 4]),
    ] {
        let mut e = engine();
        e.contexts.get_mut(&1).unwrap().last_focused = remembered;
        e.set_visible(2, &[], None);
        e.set_visible(1, &[1, 2, 3, 4], None);
        let mut click = ClickSequence::new(&e, target).unwrap();
        let mut visited = vec![];
        while let Some(window) = click.advance(&e) {
            visited.push(window);
            e.focus(window);
        }
        assert_eq!(visited, expected);
        assert_eq!(e.focused(), Some(target));
        assert_eq!(e.context().unwrap().order, [1, 2, 3, 4]);
    }
}
#[test]
fn background_click_eligibility_uses_processes_and_keeps_both_traversal_directions() {
    let mut e = engine();
    for (start, target, expected) in [(1, 4, vec![2, 3]), (4, 1, vec![3, 2])] {
        e.focus(start);
        let click = ClickSequence::new(&e, target).unwrap();
        assert_eq!(
            click.background_windows(|id| Some((id as i32, None))),
            Some(expected.clone())
        );
        // Repeated background applications must retain ordinary traversal.
        assert_eq!(
            click.background_windows(|id| Some((
                if id == 2 || id == 3 { 9 } else { id as i32 },
                None
            ))),
            None
        );
        assert!(click.background_windows(|_| Some((1, None))).is_none());
        assert!(
            click
                .background_windows(|id| Some((
                    if id == start || id == target {
                        99
                    } else {
                        id as i32
                    },
                    None
                )))
                .is_none()
        );
        assert!(
            click
                .background_windows(|id| Some((
                    id as i32,
                    Some(if id == 2 || id == 3 {
                        "background.app".into()
                    } else {
                        format!("app.{id}")
                    })
                )))
                .is_none()
        );
        // Separate processes of the same application are not background apps.
        assert!(
            click
                .background_windows(|id| Some((id as i32, Some("same.app".into()))))
                .is_none()
        );
        assert!(
            click
                .background_windows(|_| Some((1, Some("unique.app".into()))))
                .is_none()
        );
        assert!(
            click
                .background_windows(|id| (id != 2).then_some((id as i32, None)))
                .is_none()
        );
    }
    e.focus(1);
    assert!(
        ClickSequence::new(&e, 2)
            .unwrap()
            .background_windows(|id| Some((id as i32, None)))
            .is_none()
    );
    assert!(
        ClickSequence::new(&e, 1)
            .unwrap()
            .background_windows(|id| Some((id as i32, None)))
            .is_none()
    );
    e.set_visible(1, &[1, 2, 3, 4], None);
    assert_eq!(
        ClickSequence::new(&e, 4)
            .unwrap()
            .background_windows(|id| Some((id as i32, None))),
        Some(vec![1, 2, 3])
    );
}
#[test]
fn immediate_click_retains_the_original_path_for_failed_background_repairs() {
    let mut e = engine();
    e.focus(1);
    let mut click = ClickSequence::new(&e, 4).unwrap();
    e.focus(4);
    click.accept_destination();
    assert_eq!(click.selected(), Some(4));
    let mut restored = vec![];
    while let Some(id) = click.advance(&e) {
        restored.push(id);
        e.focus(id);
    }
    assert_eq!(restored, [2, 3, 4]);
    assert_eq!(e.focused(), Some(4));
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
fn frozen_engine_never_moves_or_focuses() {
    let mut e = engine();
    e.frozen = true;
    assert!(e.command(Command::FocusNext).is_empty());
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
        for key in [123, 124, 125, 126, 46] {
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
fn former_group_shortcuts_are_not_captured() {
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
}
