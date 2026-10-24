//! Invoke macOS's own Window-menu commands. Synthesized Fn events are not
//! reliably honored by macOS; never substitute our own geometry calculations.
use super::ffi::*;
use whalu::input::NativeShortcut;

pub fn perform(pid: i32, shortcut: NativeShortcut) -> Result<(), String> {
    unsafe {
        let app =
            Owned::new(AXUIElementCreateApplication(pid)).ok_or("Cannot inspect active app")?;
        AXUIElementSetMessagingTimeout(app.0, 0.15);
        let menu = attr(app.0, "AXMenuBar").ok_or("Active app has no accessible menu bar")?;
        let (_, owner) = find(menu.0, shortcut, 0, None)
            .ok_or("Active app does not expose this native tiling command")?;
        let status = AXUIElementPerformAction(owner.0, string("AXPress").0);
        if status != 0 {
            return Err(format!("Cannot validate native Window menu (AX {status})"));
        }
        let _close = OpenMenu(owner.clone());
        let (mut item, _) = find(owner.0, shortcut, 1, Some(owner.0))
            .ok_or("Native tiling command disappeared during menu validation")?;
        // AppKit validates nested Move & Resize items only when that submenu
        // opens; otherwise Return to Previous Size can retain a stale disabled state.
        if let Some(parent) = attr(item.0, "AXParent").and_then(|menu| attr(menu.0, "AXParent")) {
            if !CFEqual(parent.0, owner.0) {
                let status = AXUIElementPerformAction(parent.0, string("AXPress").0);
                if status != 0 {
                    return Err(format!(
                        "Cannot validate native tiling submenu (AX {status})"
                    ));
                }
                item = find(owner.0, shortcut, 1, Some(owner.0))
                    .ok_or("Native tiling command disappeared during submenu validation")?
                    .0;
            }
        }
        if !bool_attr(item.0, "AXEnabled") {
            return Err(format!(
                "Native tiling command {:?} is disabled for this window",
                text_attr(item.0, "AXTitle")
            ));
        }
        let status = AXUIElementPerformAction(item.0, string("AXPress").0);
        if status != 0 {
            return Err(format!("Native tiling menu action failed (AX {status})"));
        }
        Ok(())
    }
}

struct OpenMenu(Owned);
impl Drop for OpenMenu {
    fn drop(&mut self) {
        unsafe {
            AXUIElementPerformAction(self.0.0, string("AXCancel").0);
        }
    }
}

unsafe fn find(
    element: Ref,
    shortcut: NativeShortcut,
    depth: usize,
    owner: Option<Ref>,
) -> Option<(Owned, Owned)> {
    if depth > 7 {
        return None;
    }
    let owner = if depth == 1 { Some(element) } else { owner };
    if text_attr(element, "AXRole") == "AXMenuItem" {
        let key = attr(element, "AXMenuItemCmdVirtualKey").map(|value| unsafe { number(value.0) });
        let modifiers =
            attr(element, "AXMenuItemCmdModifiers").map(|value| unsafe { number(value.0) });
        if matches_native(
            shortcut,
            key,
            &text_attr(element, "AXMenuItemCmdChar"),
            modifiers,
        ) {
            return Some(unsafe { (Owned(CFRetain(element)), Owned(CFRetain(owner?))) });
        }
    }
    let children = attr(element, "AXChildren")?;
    // Window is normally near the end of the menu bar. Prefer it to editing
    // commands with a similar key equivalent in earlier application menus.
    for child in unsafe { array(children.0) }.into_iter().rev() {
        if let Some(found) = unsafe { find(child, shortcut, depth + 1, owner) } {
            return Some(found);
        }
    }
    None
}

fn matches_native(
    shortcut: NativeShortcut,
    key: Option<i64>,
    character: &str,
    modifiers: Option<i64>,
) -> bool {
    // AX encodes Shift=1, Option=2, Control=4, NoCommand=8, Function=16.
    // Require Function so unrelated app editing commands cannot match.
    let expected = 4
        | 8
        | 16
        | if shortcut.flags & (1 << 17) != 0 {
            1
        } else {
            0
        };
    let letter = match shortcut.keycode {
        3 => Some("f"),
        8 => Some("c"),
        15 => Some("r"),
        _ => None,
    };
    let matches_key = key == Some(i64::from(shortcut.keycode))
        || letter.is_some_and(|letter| character.eq_ignore_ascii_case(letter));
    matches_key && modifiers == Some(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_key_equivalents_match_without_localized_menu_titles() {
        for key in [123, 124, 125, 126] {
            for arrange in [false, true] {
                let shortcut = NativeShortcut::tiling(key, arrange);
                let modifiers = if arrange { 29 } else { 28 };
                assert!(matches_native(
                    shortcut,
                    Some(i64::from(key)),
                    "",
                    Some(modifiers)
                ));
                assert!(!matches_native(
                    shortcut,
                    Some(i64::from(key)),
                    "",
                    Some(modifiers | 2)
                ));
            }
        }
        for (key, character) in [(3, "f"), (8, "C"), (15, "r")] {
            let shortcut = NativeShortcut::tiling(key, false);
            assert!(matches_native(shortcut, None, character, Some(28)));
            assert!(!matches_native(shortcut, None, character, Some(12)));
            assert!(!matches_native(shortcut, None, "x", Some(28)));
        }
    }
}
