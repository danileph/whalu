//! Move membership through WindowServer, then let Dock animate the Space swipe.
//! Gesture events target the window’s display without moving the cursor.
use super::ffi::*;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    sel,
};
use std::{
    ffi::CStr,
    ptr::null,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

type Connection = unsafe extern "C" fn() -> i32;
type Displays = unsafe extern "C" fn(i32) -> Ref;
type Animating = unsafe extern "C" fn(i32, Ref) -> bool;
type WindowSpaces = unsafe extern "C" fn(i32, i32, Ref) -> Ref;

unsafe fn symbol<T: Copy>(name: &CStr) -> Result<T, String> {
    let pointer = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if pointer.is_null() {
        return Err(format!(
            "{} unavailable on this macOS",
            name.to_string_lossy()
        ));
    }
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}

pub fn move_adjacent(element: Ref, window: u32, forward: bool) -> Result<bool, String> {
    unsafe {
        let connection: Connection = symbol(c"SLSMainConnectionID")?;
        let displays: Displays = symbol(c"SLSCopyManagedDisplaySpaces")?;
        let animating: Animating = symbol(c"SLSManagedDisplayIsAnimating")?;
        let connection = connection();
        let Some(source) = window_space(window)? else {
            return Ok(false);
        };
        let managed = Owned::new(displays(connection)).ok_or("Cannot read desktop order")?;
        for display in array(managed.0) {
            // Stop at fullscreen neighbors instead of skipping a desktop.
            let spaces = array(dict(display, "Spaces"));
            let ids: Vec<_> = spaces
                .iter()
                .map(|space| number(dict(*space, "ManagedSpaceID")) as u64)
                .collect();
            let Some(index) = ids.iter().position(|id| *id == source) else {
                continue;
            };
            let Some(target) = adjacent(&ids, index, forward) else {
                return Ok(false);
            };
            let target_index = ids.iter().position(|id| *id == target).unwrap();
            if number(dict(spaces[index], "type")) != 0
                || number(dict(spaces[target_index], "type")) != 0
            {
                return Ok(false);
            }
            if number(dict(dict(display, "Current Space"), "ManagedSpaceID")) as u64 != source {
                return Err("Focused window is not on the active desktop".into());
            }
            let identifier = dict(display, "Display Identifier");
            let system = Owned::new(AXUIElementCreateSystemWide()).ok_or("Cannot confirm focus")?;
            AXUIElementSetMessagingTimeout(system.0, 0.05);
            let focused = attr(system.0, "AXFocusedApplication")
                .and_then(|app| attr(app.0, "AXFocusedWindow"));
            if !focused.is_some_and(|focused| CFEqual(focused.0, element)) {
                return Err("Focused window changed before Space transfer".into());
            }
            let window_number = i64::from(window);
            let window_value = Owned::new(CFNumberCreate(
                null(),
                4,
                (&window_number as *const i64).cast(),
            ))
            .ok_or("Cannot create window identity")?;
            let windows = Owned::new(CFArrayCreate(null(), &window_value.0, 1, null()))
                .ok_or("Cannot create window list")?;
            let move_class = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation")
                .ok_or("Native Space transfer unavailable")?;
            let allocated: *mut AnyObject = msg_send![move_class, alloc];
            let operation: *mut AnyObject = msg_send![allocated, initWithWindows: &*(windows.0 as *const AnyObject), spaceID: target];
            let movement =
                Retained::from_raw(operation).ok_or("Cannot create native Space transfer")?;
            validate_bridge(move_class)?;
            let rectangle = frame(element).ok_or("Cannot locate focused window display")?;
            let animation = DockSwipe::new(
                Point {
                    x: rectangle.x + rectangle.width / 2.,
                    y: rectangle.y + rectangle.height / 2.,
                },
                forward,
            )?;
            // Keep a source-only witness: merely changing WindowServer's current
            // Space pointer can leave these windows rendered on the destination.
            let witnesses: Vec<_> = onscreen()
                .into_iter()
                .filter(|id| *id != window && window_space(*id).ok() == Some(Some(source)))
                .collect();
            perform_bridge(move_class, &movement);
            let deadline = Instant::now() + Duration::from_millis(750);
            while window_space(window)? != Some(target) {
                if Instant::now() >= deadline {
                    return Err("WindowServer did not move the window".into());
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            let mut animated =
                animation.perform(connection, identifier, animating, window, &witnesses);
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let current = Owned::new(displays(connection))
                    .ok_or("Cannot verify native Space transition")?;
                let active = array(current.0).into_iter().any(|display| {
                    let id = dict(display, "Display Identifier");
                    !id.is_null()
                        && CFEqual(id, identifier)
                        && number(dict(dict(display, "Current Space"), "ManagedSpaceID")) as u64
                            == target
                });
                let running = animating(connection, identifier);
                animated |= running;
                if active && !running {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Native Space shortcut did not switch desktops ({source} -> {target}; current: {:?})",
                        array(current.0)
                            .into_iter()
                            .map(|d| number(dict(dict(d, "Current Space"), "ManagedSpaceID")))
                            .collect::<Vec<_>>()
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let visible = onscreen();
                if window_space(window)? == Some(target)
                    && visible.contains(&window)
                    && witnesses.iter().all(|id| !visible.contains(id))
                {
                    if animated {
                        ANIMATED_TRANSFERS.fetch_add(1, Ordering::Relaxed);
                    }
                    return Ok(true);
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Native Space transfer did not reveal window {window} ({source} -> {target}; membership {:?}; visible {}; source witnesses {:?})",
                        window_space(window)?,
                        visible.contains(&window),
                        witnesses
                            .iter()
                            .filter(|id| visible.contains(id))
                            .collect::<Vec<_>>()
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Ok(false)
    }
}

pub(super) fn is_onscreen(window: u32) -> bool {
    onscreen().contains(&window)
}

fn onscreen() -> Vec<u32> {
    unsafe {
        Owned::new(CGWindowListCopyWindowInfo(17, 0))
            .map(|windows| {
                array(windows.0)
                    .into_iter()
                    .filter(|entry| number(dict(*entry, "kCGWindowLayer")) == 0)
                    .map(|entry| number(dict(entry, "kCGWindowNumber")) as u32)
                    .collect()
            })
            .unwrap_or_default()
    }
}

// Private Dock gesture encoding documented by Mac Mouse Fix and dockswipe.
// https://github.com/oomol-lab/dockswipe (MIT; see THIRD_PARTY_NOTICES.md).
// This path deliberately emits no mouse move, button, or keyboard events.
struct DockSwipe {
    frames: Vec<(Owned, Owned)>,
    end: (Owned, Owned),
}
impl DockSwipe {
    unsafe fn new(point: Point, forward: bool) -> Result<Self, String> {
        unsafe {
            let mut system: libc::utsname = std::mem::zeroed();
            if libc::uname(&mut system) != 0
                || CStr::from_ptr(system.release.as_ptr())
                    .to_string_lossy()
                    .split('.')
                    .next()
                    .and_then(|major| major.parse::<u32>().ok())
                    .is_none_or(|major| major >= 26)
            {
                return Err(
                    "Native Dock gesture encoding is unsupported on this macOS version".into(),
                );
            }
            if !CGPreflightPostEventAccess() {
                return Err("Native Space animation requires Accessibility permission".into());
            }
            let direction = if forward { 1. } else { -1. };
            let frames = (0..18)
                .map(|step| {
                    Self::event(
                        point,
                        if step == 0 { 1 } else { 2 },
                        direction * (step + 1) as f64 / 12.,
                        0.,
                    )
                })
                .collect::<Result<_, _>>()?;
            let end = Self::event(point, 4, direction * 1.5, direction * (100. / 12.))?;
            Ok(Self { frames, end })
        }
    }
    unsafe fn event(
        point: Point,
        phase: i64,
        progress: f64,
        velocity: f64,
    ) -> Result<(Owned, Owned), String> {
        unsafe {
            let gesture =
                Owned::new(CGEventCreate(null())).ok_or("Cannot create Dock gesture marker")?;
            let dock = Owned::new(CGEventCreate(null())).ok_or("Cannot create Dock swipe")?;
            for (event, kind) in [(&gesture, 29.), (&dock, 30.)] {
                CGEventSetDoubleValueField(event.0, 55, kind);
                CGEventSetLocation(event.0, point);
                CGEventSetFlags(event.0, 0);
                CGEventSetIntegerValueField(event.0, 42, DOCK_INPUT_TAG);
            }
            for (field, value) in [
                (110, 23.),
                (132, phase as f64),
                (134, phase as f64),
                (124, progress),
                (123, 1.),
                (165, 1.),
                (119, f32::from_bits(1) as f64),
                (139, f32::from_bits(1) as f64),
                (129, velocity),
                (130, velocity),
            ] {
                CGEventSetDoubleValueField(dock.0, field, value);
            }
            CGEventSetIntegerValueField(dock.0, 135, i64::from((progress as f32).to_bits()));
            CGEventSetIntegerValueField(dock.0, 136, 0);
            Ok((dock, gesture))
        }
    }
    unsafe fn post(pair: &(Owned, Owned)) {
        unsafe {
            CGEventPost(1, pair.0.0);
            CGEventPost(1, pair.1.0);
        }
    }
    unsafe fn perform(
        &self,
        connection: i32,
        display: Ref,
        animating: Animating,
        window: u32,
        witnesses: &[u32],
    ) -> bool {
        unsafe {
            let mut observed = false;
            for pair in &self.frames {
                Self::post(pair);
                std::thread::sleep(Duration::from_millis(8));
                // Interactive Dock swipes do not consistently set the managed
                // display's animation bit. During a real partial reveal, both
                // the destination window and source-only witnesses composite;
                // the final visibility check requires the witnesses to vanish.
                let visible = onscreen();
                observed |= animating(connection, display)
                    || (visible.contains(&window)
                        && witnesses.iter().any(|id| visible.contains(id)));
            }
            Self::post(&self.end);
            // Repeat Ended after Dock's native completion to avoid a dropped
            // terminal gesture leaving the desktop caught between Spaces.
            std::thread::sleep(Duration::from_millis(200));
            Self::post(&self.end);
            observed
        }
    }
}
static ANIMATED_TRANSFERS: AtomicUsize = AtomicUsize::new(0);
pub(super) fn animated_transfers() -> usize {
    ANIMATED_TRANSFERS.load(Ordering::Relaxed)
}
pub(super) const DOCK_INPUT_TAG: i64 = 0x5748_414c_5544;

fn validate_bridge(class: &AnyClass) -> Result<(), String> {
    let method = class
        .instance_method(sel!(performWithWMBridgeDelegate))
        .ok_or("Native Space operation unavailable")?;
    match method.return_type().to_bytes() {
        b"v" | b"@" => Ok(()),
        _ => Err("Unsupported native Space operation signature".into()),
    }
}
unsafe fn perform_bridge(class: &AnyClass, operation: &AnyObject) {
    unsafe {
        // Validated before any write. This private selector returns void on
        // macOS 26.2, while other releases describe an object result.
        if class
            .instance_method(sel!(performWithWMBridgeDelegate))
            .unwrap()
            .return_type()
            .to_bytes()
            == b"v"
        {
            let _: () = msg_send![operation, performWithWMBridgeDelegate];
        } else {
            let _: *mut AnyObject = msg_send![operation, performWithWMBridgeDelegate];
        }
    }
}

/// Read-only membership for the native fixture's asynchronous follow check.
pub(super) fn window_space(window: u32) -> Result<Option<u64>, String> {
    unsafe {
        let connection: Connection = symbol(c"SLSMainConnectionID")?;
        let query: WindowSpaces = symbol(c"SLSCopySpacesForWindows")?;
        let value = i64::from(window);
        let number = Owned::new(CFNumberCreate(null(), 4, (&value as *const i64).cast()))
            .ok_or("Cannot create window identity")?;
        let windows = Owned::new(CFArrayCreate(null(), &number.0, 1, null()))
            .ok_or("Cannot create window list")?;
        let spaces =
            Owned::new(query(connection(), 7, windows.0)).ok_or("Cannot read window Spaces")?;
        let ids = array(spaces.0);
        Ok((ids.len() == 1).then(|| super::ffi::number(ids[0]) as u64))
    }
}

fn adjacent(desktops: &[u64], index: usize, forward: bool) -> Option<u64> {
    let target = if forward {
        index.checked_add(1)?
    } else {
        index.checked_sub(1)?
    };
    desktops.get(target).copied()
}

#[cfg(test)]
mod tests {
    use super::adjacent;
    #[test]
    fn desktop_moves_stop_at_display_boundaries() {
        assert_eq!(adjacent(&[10, 11, 12], 0, false), None);
        assert_eq!(adjacent(&[10, 11, 12], 1, false), Some(10));
        assert_eq!(adjacent(&[10, 11, 12], 1, true), Some(12));
        assert_eq!(adjacent(&[10, 11, 12], 2, true), None);
        assert_eq!(adjacent(&[20], 0, true), None);
    }
}
