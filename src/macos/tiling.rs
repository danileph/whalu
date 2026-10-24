//! Invoke macOS's own Window-menu commands. Synthesized Fn events are not
//! reliably honored by macOS; never substitute our own geometry calculations.
use super::ffi::*;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{
        OnceLock,
        mpsc::{SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};
use whalu::input::NativeShortcut;

thread_local! {
    static ACTIONS: RefCell<BTreeMap<(i32, u16, u64), Owned>> = const { RefCell::new(BTreeMap::new()) };
}

// AX window handles are immutable CF objects and support cross-thread messaging.
// This wrapper owns exactly one retain while a request waits/runs on the worker.
struct Target(usize);
impl Drop for Target {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0 as Ref);
        }
    }
}
enum Operation {
    Tile(NativeShortcut),
    MoveSpace { window: u32, forward: bool },
}
struct Request {
    pid: i32,
    window: Target,
    operation: Operation,
}
static WORKER: OnceLock<Result<SyncSender<Request>, String>> = OnceLock::new();

pub fn submit(pid: i32, window: Ref, shortcut: NativeShortcut) -> Result<(), String> {
    enqueue(pid, window, Operation::Tile(shortcut))
}
pub fn submit_space(pid: i32, element: Ref, window: u32, forward: bool) -> Result<(), String> {
    enqueue(pid, element, Operation::MoveSpace { window, forward })
}
fn enqueue(pid: i32, window: Ref, operation: Operation) -> Result<(), String> {
    let sender = WORKER
        .get_or_init(|| {
            let (sender, receiver) = sync_channel::<Request>(8);
            std::thread::Builder::new()
                .name("whalu-native-tiling".into())
                .spawn(move || {
                    for request in receiver {
                        objc2::rc::autoreleasepool(|_| {
                            let target = Some(request.window.0 as Ref);
                            let result = match request.operation {
                                Operation::Tile(shortcut) => {
                                    perform_guarded(request.pid, shortcut, target)
                                }
                                Operation::MoveSpace { window, forward } => {
                                    if still_focused(request.pid, target) {
                                        move_and_focus(
                                            request.pid,
                                            request.window.0 as Ref,
                                            window,
                                            forward,
                                        )
                                    } else {
                                        Err("Focused window changed; skipped stale Space request"
                                            .into())
                                    }
                                }
                            };
                            if let Err(error) = result {
                                eprintln!("Native window command: {error}");
                            }
                        });
                        super::enqueue(super::Event::Dirty);
                    }
                })
                .map_err(|error| error.to_string())?;
            Ok(sender)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let window = Target(unsafe { CFRetain(window) } as usize);
    sender
        .try_send(Request {
            pid,
            window,
            operation,
        })
        .map_err(|_| "Native window command queue is busy".into())
}

fn move_and_focus(pid: i32, element: Ref, window: u32, forward: bool) -> Result<(), String> {
    if !super::spaces::move_adjacent(window, forward)? {
        return Ok(());
    }
    unsafe {
        let app = Owned::new(AXUIElementCreateApplication(pid))
            .ok_or("Cannot focus transferred window")?;
        AXUIElementSetMessagingTimeout(app.0, 0.05);
        AXUIElementSetMessagingTimeout(element, 0.05);
        AXUIElementSetAttributeValue(app.0, string("AXFocusedWindow").0, element);
        AXUIElementSetAttributeValue(element, string("AXFocused").0, kCFBooleanTrue);
        let main = AXUIElementSetAttributeValue(element, string("AXMain").0, kCFBooleanTrue);
        let raised = AXUIElementPerformAction(element, string("AXRaise").0);
        AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
        if main != 0 && raised != 0 {
            return Err(format!(
                "Could not focus transferred window (AX {main}/{raised})"
            ));
        }
    }
    Ok(())
}

pub fn perform(pid: i32, shortcut: NativeShortcut) -> Result<(), String> {
    perform_guarded(pid, shortcut, None)
}

fn still_focused(pid: i32, window: Option<Ref>) -> bool {
    let Some(window) = window else {
        return true;
    };
    unsafe {
        let Some(system) = Owned::new(AXUIElementCreateSystemWide()) else {
            return false;
        };
        AXUIElementSetMessagingTimeout(system.0, 0.025);
        let Some(app) = attr(system.0, "AXFocusedApplication") else {
            return false;
        };
        let mut active_pid = 0;
        AXUIElementGetPid(app.0, &mut active_pid) == 0
            && active_pid == pid
            && attr(app.0, "AXFocusedWindow").is_some_and(|focused| CFEqual(focused.0, window))
    }
}

fn perform_guarded(pid: i32, shortcut: NativeShortcut, window: Option<Ref>) -> Result<(), String> {
    if shortcut.keycode == 8 {
        // Native Center preserves the current dimensions, including a filled
        // rectangle. First ask macOS to restore its own pre-tiling size.
        perform_one(pid, NativeShortcut::tiling(15, false), window)?;
        let app =
            unsafe { Owned::new(AXUIElementCreateApplication(pid)) }.ok_or("No centering app")?;
        let target = attr(app.0, "AXFocusedWindow").ok_or("No centering window")?;
        let start = Instant::now();
        let mut last = frame(target.0);
        let mut stable = Instant::now();
        while start.elapsed() < Duration::from_millis(750) {
            if !still_focused(pid, window) {
                return Err("Focus changed while restoring for Center".into());
            }
            std::thread::sleep(Duration::from_millis(20));
            let current = frame(target.0);
            if current != last {
                stable = Instant::now();
                last = current;
            }
            if start.elapsed() >= Duration::from_millis(450)
                && stable.elapsed() >= Duration::from_millis(60)
            {
                break;
            }
        }
    }
    perform_one(pid, shortcut, window)
}

fn perform_one(pid: i32, shortcut: NativeShortcut, window: Option<Ref>) -> Result<(), String> {
    if !still_focused(pid, window) {
        return Err("Focused window changed; skipped stale request".into());
    }
    let key = (pid, shortcut.keycode, shortcut.flags);
    if let Some(item) = ACTIONS.with(|cache| cache.borrow().get(&key).cloned()) {
        if press(&item) == 0 {
            return Ok(());
        }
        // App relaunches and rebuilt menus invalidate AX references. Retry only
        // after dropping the stale reference, never by opening a menu.
        ACTIONS.with(|cache| cache.borrow_mut().remove(&key));
    }
    unsafe {
        let app =
            Owned::new(AXUIElementCreateApplication(pid)).ok_or("Cannot inspect active app")?;
        AXUIElementSetMessagingTimeout(app.0, 0.025);
        let deadline = Instant::now() + Duration::from_millis(750);
        let item = loop {
            // AppKit can populate its native menu entries lazily on first AX
            // access. Retry fresh snapshots briefly, always on the worker.
            if let Some(menu) = attr(app.0, "AXMenuBar") {
                let mut remaining = 256;
                if let Some(item) = find(menu.0, shortcut, 0, &mut remaining, deadline) {
                    break item;
                }
            }
            if Instant::now() >= deadline {
                return Err("Active app does not expose this native tiling command".into());
            }
            if !still_focused(pid, window) {
                return Err("Focused window changed during native action lookup".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        // AXEnabled can be an unvalidated snapshot. Dispatch only the leaf;
        // AppKit owns command validation. No menu enters its tracking loop.
        if !still_focused(pid, window) {
            return Err("Focused window changed during native action lookup".into());
        }
        let status = press(&item);
        if status != 0 {
            return Err(format!("Native tiling action failed (AX {status})"));
        }
        ACTIONS.with(|cache| {
            let mut cache = cache.borrow_mut();
            // Bound retained references when applications repeatedly relaunch.
            if cache.len() >= 128 {
                cache.clear();
            }
            cache.insert(key, item);
        });
        Ok(())
    }
}

fn press(item: &Owned) -> i32 {
    if std::env::var_os("WHALU_TRACE").is_some() {
        eprintln!("Native leaf action: {:?}", text_attr(item.0, "AXTitle"));
    }
    unsafe {
        AXUIElementSetMessagingTimeout(item.0, 0.05);
        AXUIElementPerformAction(item.0, string("AXPress").0)
    }
}

unsafe fn find(
    element: Ref,
    shortcut: NativeShortcut,
    depth: usize,
    remaining: &mut usize,
    deadline: Instant,
) -> Option<Owned> {
    if depth > 7 || *remaining == 0 || Instant::now() >= deadline {
        return None;
    }
    *remaining -= 1;
    unsafe {
        AXUIElementSetMessagingTimeout(element, 0.025);
    }
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
            return Some(unsafe { Owned(CFRetain(element)) });
        }
    }
    let children = attr(element, "AXChildren")?;
    for child in unsafe { array(children.0) }.into_iter().rev() {
        if let Some(found) = unsafe { find(child, shortcut, depth + 1, remaining, deadline) } {
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
