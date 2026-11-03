use whalu::{
    core::Command,
    input::{ButtonRoute, OverviewButtons},
};

#[test]
fn back_and_forward_navigate_once_only_when_overview_is_visible() {
    let mut buttons = OverviewButtons::default();
    for (button, command) in [(3, Command::FocusNext), (4, Command::FocusPrevious)] {
        assert_eq!(buttons.route(false, 25, button), ButtonRoute::Pass);
        assert_eq!(buttons.route(false, 26, button), ButtonRoute::Pass);
        assert_eq!(
            buttons.route(true, 25, button),
            ButtonRoute::Consume(Some(command))
        );
        assert_eq!(buttons.route(true, 25, button), ButtonRoute::Consume(None));
        assert_eq!(buttons.route(true, 26, button), ButtonRoute::Consume(None));
        assert_eq!(buttons.route(true, 26, button), ButtonRoute::Pass);
    }
}

#[test]
fn ownership_follows_button_down_through_dismissal_and_reopening() {
    let mut buttons = OverviewButtons::default();
    assert_eq!(
        buttons.route(true, 25, 3),
        ButtonRoute::Consume(Some(Command::FocusNext))
    );
    assert_eq!(buttons.route(false, 27, 3), ButtonRoute::Consume(None));
    assert_eq!(buttons.route(false, 26, 3), ButtonRoute::Consume(None));
    assert_eq!(buttons.route(false, 25, 4), ButtonRoute::Pass);
    assert_eq!(buttons.route(true, 25, 4), ButtonRoute::Pass);
    assert_eq!(buttons.route(true, 27, 4), ButtonRoute::Pass);
    assert_eq!(buttons.route(true, 26, 4), ButtonRoute::Pass);
    assert_eq!(
        buttons.route(true, 25, 4),
        ButtonRoute::Consume(Some(Command::FocusPrevious))
    );
}

#[test]
fn unrelated_buttons_and_unowned_releases_always_pass() {
    let mut buttons = OverviewButtons::default();
    for visible in [false, true] {
        for button in [-1, 0, 1, 2, 5, 8] {
            for kind in [25, 26, 27] {
                assert_eq!(buttons.route(visible, kind, button), ButtonRoute::Pass);
            }
        }
        for button in [3, 4] {
            assert_eq!(buttons.route(visible, 26, button), ButtonRoute::Pass);
        }
    }
}

#[test]
fn simultaneous_side_buttons_have_independent_ownership() {
    let mut buttons = OverviewButtons::default();
    buttons.route(true, 25, 3);
    buttons.route(false, 25, 4);
    assert_eq!(buttons.route(true, 26, 4), ButtonRoute::Pass);
    assert_eq!(buttons.route(false, 26, 3), ButtonRoute::Consume(None));
}
