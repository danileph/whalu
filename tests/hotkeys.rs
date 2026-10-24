use whalu::{
    config::Config,
    core::{Command, Effect, Engine, Mode, Window},
    geometry::Rect,
    input::{KeyRoute, Modifiers, NativeShortcut, ShortcutRouter, route},
};

const CONTROL: u64 = 1 << 18;
const OPTION: u64 = 1 << 19;
const COMMAND: u64 = 1 << 20;
const SHIFT: u64 = 1 << 17;
const FN: u64 = 1 << 23;

#[test]
fn command_switching_owns_repeat_release_overview_and_wheel_navigation() {
    use whalu::scroll::{WheelRoute, WheelRouter};

    let config = Config::default();
    let command = Modifiers::from_flags(COMMAND);
    let released = Modifiers::default();
    for (key, action) in [(48, Command::FocusNext), (50, Command::FocusPrevious)] {
        let mut router = ShortcutRouter::default();
        let mut wheel = WheelRouter::default();
        assert!(!router.overview_held(command, config.overlay_modifiers));
        assert_eq!(
            router.key(&config.bindings, key, COMMAND, true),
            KeyRoute::Consume(Some(action))
        );
        assert!(router.overview_held(command, config.overlay_modifiers));
        // A held key continues to navigate and survives configuration changes.
        assert_eq!(
            router.key(&[], key, COMMAND, true),
            KeyRoute::Consume(Some(action))
        );
        assert_eq!(
            wheel.route(
                router.overview_held(command, config.overlay_modifiers),
                -1.,
                false,
                0,
                0,
                0.,
            ),
            WheelRoute::Navigate(1.)
        );
        // Releasing Tab/grave first retains the session while Command is down.
        assert_eq!(
            router.key(&[], key, COMMAND, false),
            KeyRoute::Consume(None)
        );
        assert!(router.overview_held(command, config.overlay_modifiers));
        assert!(!router.overview_held(released, config.overlay_modifiers));
        // Releasing Command first must not hide a still-held navigation key.
        router.key(&config.bindings, key, COMMAND, true);
        assert!(router.overview_held(released, config.overlay_modifiers));
        assert_eq!(
            router.key(&[], key, 0, true),
            KeyRoute::Consume(Some(action))
        );
        assert!(router.overview_held(released, config.overlay_modifiers));
        assert_eq!(router.key(&[], key, 0, false), KeyRoute::Consume(None));
        assert!(!router.overview_held(released, config.overlay_modifiers));
        assert_eq!(router.key(&[], key, 0, false), KeyRoute::Pass);
        assert_eq!(wheel.route(false, -1., false, 0, 0, 1.), WheelRoute::Pass);
    }
}

#[test]
fn overview_ownership_preserves_custom_bindings_modifiers_and_simultaneous_keys() {
    let config = Config::default();
    let mut router = ShortcutRouter::default();
    let command = Modifiers::from_flags(COMMAND);
    for flags in [0, COMMAND | SHIFT, COMMAND | OPTION, FN | COMMAND] {
        assert_eq!(
            router.key(&config.bindings, 48, flags, true),
            KeyRoute::Pass
        );
        assert!(!router.overview_held(command, config.overlay_modifiers));
    }
    assert_eq!(router.key(&[], 48, COMMAND, true), KeyRoute::Pass);
    assert!(!router.overview_held(command, config.overlay_modifiers));
    assert!(router.overview_held(
        Modifiers::from_flags(CONTROL | OPTION),
        config.overlay_modifiers
    ));
    router.key(&config.bindings, 48, COMMAND, true);
    router.key(&config.bindings, 50, COMMAND, true);
    assert!(router.overview_held(Modifiers::default(), config.overlay_modifiers));
    router.key(&[], 48, 0, false);
    assert!(router.overview_held(Modifiers::default(), config.overlay_modifiers));
    router.key(&[], 50, 0, false);
    assert!(!router.overview_held(command, config.overlay_modifiers));
    // A disabled/recreated tap starts with no stale hold or switch session.
    router.key(&config.bindings, 48, COMMAND, true);
    router = ShortcutRouter::default();
    assert!(!router.overview_held(command, config.overlay_modifiers));
}

#[test]
fn screenshot_shortcuts_route_to_stack_spaces_and_native_tiling() {
    let config = Config::default();
    config.validate().unwrap();
    assert_eq!(config.bindings.len(), 37);
    for (keys, command) in [
        (&[126, 40][..], Command::StackUp),
        (&[125, 38][..], Command::StackDown),
        (&[123, 4][..], Command::MoveSpace(false)),
        (&[124, 37][..], Command::MoveSpace(true)),
    ] {
        for &key in keys {
            assert_eq!(
                route(
                    &config.bindings,
                    key,
                    Modifiers::from_flags(CONTROL | OPTION | COMMAND)
                ),
                Some(command)
            );
        }
    }
    for (keys, native_key, arrange) in [
        (&[123, 4][..], 123, false),
        (&[124, 37][..], 124, false),
        (&[126, 40][..], 126, false),
        (&[125, 38][..], 125, false),
        (&[46][..], 3, false),
        (&[34][..], 8, false),
        (&[45][..], 15, false),
        (&[123, 4][..], 123, true),
        (&[124, 37][..], 124, true),
        (&[126, 40][..], 126, true),
        (&[125, 38][..], 125, true),
    ] {
        for &key in keys {
            let mut router = ShortcutRouter::default();
            let flags = CONTROL
                | COMMAND
                | if arrange { SHIFT } else { 0 }
                | if key >= 123 { FN | (1 << 21) } else { 0 };
            let expected = KeyRoute::Native(NativeShortcut {
                keycode: native_key,
                flags: CONTROL | FN | if arrange { SHIFT } else { 0 },
            });
            assert_eq!(router.key(&config.bindings, key, flags, true), expected);
            // Hold/repeat keeps the original action even after modifiers/config change.
            assert_eq!(router.key(&[], key, 0, true), expected);
            assert_eq!(router.key(&[], key, 0, false), expected);
            assert_eq!(router.key(&[], key, 0, false), KeyRoute::Pass);
        }
    }
}

#[test]
fn native_navigation_and_tiling_remain_available_without_whalu_interception() {
    let config = Config::default();
    let mut router = ShortcutRouter::default();
    for (keys, flags) in [
        (&[48, 50][..], COMMAND | SHIFT),
        (&[123, 124, 125, 126][..], CONTROL),
        (&[123, 124, 125, 126, 3, 8, 15][..], FN | CONTROL),
        (&[123, 124, 125, 126][..], FN | CONTROL | SHIFT),
        (&[49][..], CONTROL | OPTION),
    ] {
        for &key in keys {
            assert_eq!(
                router.key(&config.bindings, key, flags, true),
                KeyRoute::Pass
            );
            assert_eq!(router.key(&config.bindings, key, 0, false), KeyRoute::Pass);
        }
    }
    assert_eq!(
        router.key(&config.bindings, 40, CONTROL | OPTION | COMMAND, true),
        KeyRoute::Consume(Some(Command::StackUp))
    );
    assert_eq!(router.key(&[], 40, 0, false), KeyRoute::Consume(None));
}

#[test]
fn control_option_arrows_and_vim_keys_navigate_without_conflicting_with_stack_edits() {
    let config = Config::default();
    for (keys, command) in [
        (&[126, 40][..], Command::FocusPrevious),
        (&[125, 38][..], Command::FocusNext),
        (&[123, 4][..], Command::AppPrevious),
        (&[124, 37][..], Command::AppNext),
    ] {
        for &key in keys {
            let mut router = ShortcutRouter::default();
            let flags = CONTROL | OPTION | if key >= 123 { FN | (1 << 21) } else { 0 };
            assert_eq!(
                router.key(&config.bindings, key, flags, true),
                KeyRoute::Consume(Some(command))
            );
            assert_eq!(
                router.key(&config.bindings, key, 0, false),
                KeyRoute::Consume(None)
            );
        }
    }
    assert_eq!(
        route(
            &config.bindings,
            126,
            Modifiers::from_flags(CONTROL | OPTION | COMMAND)
        ),
        Some(Command::StackUp)
    );
}

fn engine() -> Engine {
    let mut engine = Engine::new(8.);
    for id in 1..=4 {
        engine.windows.insert(
            id,
            Window {
                id,
                app_id: "test".into(),
                app_name: "Test".into(),
                title: id.to_string(),
                display: 1,
                frame: Rect::new(10., 10., 400., 300.),
                mode: Mode::Magnetic,
                last_floating_frame: None,
            },
        );
    }
    engine.set_display_visible(1, 1, &[1, 2, 3, 4], Some(3));
    engine
}

#[test]
fn reorder_keeps_focus_and_geometry_and_survives_reconciliation_and_hidden_members() {
    let mut engine = engine();
    let frames: Vec<_> = engine.windows.values().map(|window| window.frame).collect();
    assert!(engine.command(Command::StackUp).is_empty());
    assert_eq!(engine.context().unwrap().order, [1, 3, 2, 4]);
    assert_eq!(engine.focused(), Some(3));
    engine.set_display_visible(1, 1, &[4, 2, 3, 1], Some(3));
    assert_eq!(engine.context().unwrap().order, [1, 3, 2, 4]);
    engine.set_display_visible(1, 1, &[1, 3, 4], Some(3));
    engine.command(Command::StackDown);
    assert_eq!(engine.context().unwrap().order, [1, 4, 3]);
    engine.set_display_visible(1, 1, &[1, 2, 3, 4], Some(3));
    assert_eq!(engine.context().unwrap().order, [1, 4, 2, 3]);
    assert_eq!(engine.command(Command::FocusPrevious), [Effect::Focus(2)]);
    assert_eq!(
        engine
            .windows
            .values()
            .map(|window| window.frame)
            .collect::<Vec<_>>(),
        frames
    );
    engine.command(Command::StackDown);
    assert_eq!(engine.context().unwrap().order, [1, 4, 2, 3]);
    engine.frozen = true;
    engine.command(Command::StackUp);
    assert_eq!(engine.context().unwrap().order, [1, 4, 2, 3]);
}
