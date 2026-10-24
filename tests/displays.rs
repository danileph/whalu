use whalu::{
    core::{Command, Effect, Engine, Mode, Window},
    geometry::Rect,
    gestures::Axis,
    navigation::{ClickSequence, ScrubSession},
    visibility::DisplayContexts,
};

fn engine() -> Engine {
    let mut engine = Engine::new(8.);
    engine.bounds.insert(1, Rect::new(0., 30., 1440., 850.));
    engine
        .bounds
        .insert(2, Rect::new(-1920., -200., 1920., 1080.));
    for (id, display) in [(1, 1), (2, 1), (3, 2), (4, 2), (5, 1), (6, 1)] {
        engine.windows.insert(
            id,
            Window {
                id,
                display,
                app_id: "Editor".into(),
                app_name: "Editor".into(),
                title: format!("Document {id}"),
                frame: Rect::new(if display == 1 { 50. } else { -1800. }, 50., 600., 700.),
                mode: Mode::Magnetic,
                last_floating_frame: None,
            },
        );
    }
    engine
}

fn activate(
    engine: &mut Engine,
    tracker: &mut DisplayContexts,
    display: u32,
    visible: &[u64],
    focused: Option<u64>,
    space_changed: bool,
) -> u64 {
    tracker.retain(
        &engine
            .windows
            .values()
            .map(|window| (window.id, window.display))
            .collect(),
    );
    let local: Vec<_> = visible
        .iter()
        .copied()
        .filter(|id| engine.windows[id].display == display)
        .collect();
    let context = tracker.resolve(display, &local, space_changed);
    // Pass the complete snapshot here to also exercise the engine's boundary.
    engine.set_display_visible(display, context, visible, focused);
    context
}

#[test]
fn overview_keyboard_gestures_and_clicks_share_the_active_monitors_stack() {
    let mut engine = engine();
    let mut tracker = DisplayContexts::default();
    let visible = [1, 3, 2, 4];
    for (display, first, second, foreign) in [(1, 1, 2, 3), (2, 3, 4, 1)] {
        activate(
            &mut engine,
            &mut tracker,
            display,
            &visible,
            Some(first),
            false,
        );
        assert_eq!(engine.context().unwrap().order, [first, second]);
        for command in [
            Command::FocusNext,
            Command::FocusPrevious,
            Command::AppNext,
            Command::AppPrevious,
        ] {
            assert_eq!(engine.command(command), [Effect::Focus(second)]);
        }
        for axis in [Axis::Vertical, Axis::Horizontal] {
            let mut session = ScrubSession::new(&engine, axis).unwrap();
            assert_eq!(session.update(100), Some(second));
            assert_eq!(session.update(-100), Some(first));
        }
        assert!(ClickSequence::new(&engine, foreign).is_none());
        engine.focus(foreign);
        assert_eq!(engine.focused(), Some(first));
    }
}

#[test]
fn switching_spaces_on_one_monitor_does_not_merge_the_other_monitors_windows() {
    let mut engine = engine();
    let mut tracker = DisplayContexts::default();
    let space_a = activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], Some(2), false);
    let other = activate(&mut engine, &mut tracker, 2, &[1, 2, 3, 4], Some(4), false);
    assert_ne!(space_a, other);

    // Monitor 2 stays on [3, 4] while monitor 1 switches from [1, 2] to [5, 6].
    let space_b = activate(&mut engine, &mut tracker, 1, &[5, 6, 3, 4], Some(5), true);
    assert_ne!(space_b, space_a);
    assert_ne!(space_b, other);
    assert_eq!(engine.context().unwrap().order, [5, 6]);
    assert_eq!(
        activate(&mut engine, &mut tracker, 2, &[5, 6, 3, 4], None, true),
        other
    );
    assert_eq!(engine.command(Command::FocusNext), [Effect::Focus(4)]);

    assert_eq!(
        activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], None, true),
        space_a
    );
    assert_eq!(engine.command(Command::FocusNext), [Effect::Focus(2)]);
    assert_eq!(engine.context().unwrap().order, [1, 2]);
}

#[test]
fn empty_monitor_never_borrows_focus_or_windows_from_another_monitor() {
    let mut engine = engine();
    let mut tracker = DisplayContexts::default();
    activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], Some(1), false);
    let empty = activate(&mut engine, &mut tracker, 2, &[1, 2], Some(1), true);
    assert!(engine.context().unwrap().order.is_empty());
    assert_eq!(engine.display(), Some(2));
    assert_eq!(engine.focused(), None);
    assert!(engine.command(Command::FocusNext).is_empty());
    assert!(ScrubSession::new(&engine, Axis::Vertical).is_none());

    let other_empty = activate(&mut engine, &mut tracker, 1, &[3, 4], None, true);
    assert_ne!(empty, other_empty);
    assert_eq!(engine.display(), Some(1));
    assert!(engine.context().unwrap().order.is_empty());
}

#[test]
fn moving_a_window_between_monitors_updates_membership_without_mixing_spaces() {
    let mut engine = engine();
    let mut tracker = DisplayContexts::default();
    let original = activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], Some(1), false);
    activate(&mut engine, &mut tracker, 2, &[1, 2, 3, 4], Some(3), false);
    engine.windows.get_mut(&2).unwrap().display = 2;
    activate(&mut engine, &mut tracker, 2, &[1, 2, 3, 4], Some(2), false);
    assert_eq!(engine.context().unwrap().order, [3, 4, 2]);
    assert_eq!(
        activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], Some(1), false),
        original
    );
    assert_eq!(engine.context().unwrap().order, [1]);
    assert!(engine.command(Command::FocusNext).is_empty());

    // The moved window must not identify its former monitor's Space by itself.
    engine.windows.get_mut(&1).unwrap().display = 2;
    let replacement = activate(&mut engine, &mut tracker, 1, &[5, 6], Some(5), true);
    assert_ne!(original, replacement);
    assert_eq!(engine.context().unwrap().order, [5, 6]);
}

#[test]
fn monitor_switch_invalidates_click_travel_even_when_both_spaces_stay_visible() {
    let mut engine = engine();
    let mut tracker = DisplayContexts::default();
    activate(&mut engine, &mut tracker, 1, &[1, 2, 3, 4], Some(1), false);
    let mut click = ClickSequence::new(&engine, 2).unwrap();
    engine.focus(2);
    activate(&mut engine, &mut tracker, 2, &[1, 2, 3, 4], Some(3), false);
    assert_eq!(click.advance(&engine), None);
    assert!(!click.pending());
}
