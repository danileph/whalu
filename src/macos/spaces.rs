//! Carry a held title bar through Dock’s native Space transition.
//! WindowServer queries only read desktop order and verify the result.
//! Never set the current-Space pointer: that does not complete a native switch.
use super::ffi::*;
use std::{
    ffi::CStr,
    ptr::null,
    time::{Duration, Instant},
};

type Connection = unsafe extern "C" fn() -> i32;
type Displays = unsafe extern "C" fn(i32) -> Ref;
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

// Tag our native input so Whalu does not treat its own grab or Space key as
// user navigation. Other apps and Dock still receive these events normally.
pub(super) const INPUT_TAG: i64 = 0x5748_414c_5553;

pub fn move_adjacent(element: Ref, window: u32, forward: bool) -> Result<bool, String> {
    unsafe {
        let connection: Connection = symbol(c"SLSMainConnectionID")?;
        let displays: Displays = symbol(c"SLSCopyManagedDisplaySpaces")?;
        let connection = connection();
        let Some(source) = window_space(window)? else {
            return Ok(false);
        };
        let managed = Owned::new(displays(connection)).ok_or("Cannot read desktop order")?;
        for display in array(managed.0) {
            // Native arrow navigation visits fullscreen Spaces too. Never skip
            // one and accidentally send the window to a different desktop.
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
            let (key, flags) = space_shortcut(forward)?;
            if !CGPreflightPostEventAccess() {
                return Err(
                    "Native input posting requires Accessibility permission for this executable"
                        .into(),
                );
            }
            let deadline = Instant::now() + Duration::from_secs(1);
            while CGEventSourceFlagsState(1) & 0x1e0000 != 0 || CGEventSourceButtonState(1, 0) {
                if Instant::now() >= deadline {
                    return Err(
                        "Release modifiers and mouse before moving a window between Spaces".into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let system = Owned::new(AXUIElementCreateSystemWide()).ok_or("Cannot confirm focus")?;
            AXUIElementSetMessagingTimeout(system.0, 0.05);
            let focused = attr(system.0, "AXFocusedApplication")
                .and_then(|app| attr(app.0, "AXFocusedWindow"));
            if !focused.is_some_and(|focused| CFEqual(focused.0, element)) {
                return Err("Focused window changed before native drag".into());
            }
            let point = grab_point(element)?;
            // Keep a source-only witness: merely changing WindowServer's current
            // Space pointer can leave these windows rendered on the destination.
            let witnesses: Vec<_> = onscreen()
                .into_iter()
                .filter(|id| *id != window && window_space(*id).ok() == Some(Some(source)))
                .collect();
            let drag = Grab::begin(point)?;
            std::thread::sleep(Duration::from_millis(100));
            drag.key(key, flags)?;
            // Dock owns the slide; keep the title bar held until it settles.
            // The guard releases on every error path as well.
            std::thread::sleep(Duration::from_millis(650));
            drop(drag);
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
                if active {
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
                    return Ok(true);
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Native drag did not carry window {window} ({source} -> {target}; membership {:?}; visible {}; source witnesses {:?})",
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

fn space_shortcut(forward: bool) -> Result<(u16, u64), String> {
    // The effective default includes SecondaryFn for arrows, although System
    // Settings displays only Control. Preferences omit these default values.
    // Query the registered shortcut so custom bindings work as well.
    type Get = unsafe extern "C" fn(i32, *mut u16, *mut u16, *mut u32) -> i32;
    type Enabled = unsafe extern "C" fn(i32) -> bool;
    unsafe {
        let get: Get = symbol(c"CGSGetSymbolicHotKeyValue")?;
        let enabled: Enabled = symbol(c"CGSIsSymbolicHotKeyEnabled")?;
        let id = if forward { 81 } else { 79 };
        if !enabled(id) {
            return Err(
                "Enable macOS Move left/right a Space shortcuts in Keyboard settings".into(),
            );
        }
        let (mut equivalent, mut key, mut flags) = (0, 0, 0);
        if get(id, &mut equivalent, &mut key, &mut flags) != 0
            || key == u16::MAX
            || (key == 0 && equivalent == u16::MAX)
        {
            return Err("Cannot read the configured macOS Space shortcut".into());
        }
        Ok((key, u64::from(flags)))
    }
}

fn grab_point(window: Ref) -> Result<Point, String> {
    let rect = frame(window).ok_or("Cannot read focused window frame")?;
    if bool_attr(window, "AXFullScreen") {
        return Err("Cannot drag a fullscreen window between desktops".into());
    }
    unsafe {
        let system = Owned::new(AXUIElementCreateSystemWide()).ok_or("Cannot inspect title bar")?;
        AXUIElementSetMessagingTimeout(system.0, 0.05);
        for fraction in [0.5, 0.75, 0.25] {
            let point = Point {
                x: rect.x + rect.width * fraction,
                y: rect.y + 16.,
            };
            let mut hit = null();
            if AXUIElementCopyElementAtPosition(system.0, point.x as f32, point.y as f32, &mut hit)
                != 0
            {
                continue;
            }
            let Some(hit) = Owned::new(hit) else { continue };
            let own_window = CFEqual(hit.0, window)
                || attr(hit.0, "AXWindow").is_some_and(|owner| CFEqual(owner.0, window));
            if own_window
                && matches!(
                    text_attr(hit.0, "AXRole").as_str(),
                    "AXWindow" | "AXStaticText" | "AXGroup"
                )
            {
                return Ok(point);
            }
        }
    }
    Err("No unobstructed title-bar grab point for the focused window".into())
}

struct Grab {
    source: Owned,
    original: Point,
    point: Point,
    release: Owned,
}
impl Grab {
    unsafe fn begin(point: Point) -> Result<Self, String> {
        unsafe {
            let source =
                Owned::new(CGEventSourceCreate(1)).ok_or("Cannot create native input source")?;
            let event = Owned::new(CGEventCreate(null())).ok_or("Cannot read pointer")?;
            let original = CGEventGetLocation(event.0);
            let release = Owned::new(CGEventCreateMouseEvent(source.0, 2, point, 0))
                .ok_or("Cannot create mouse release")?;
            let down = Owned::new(CGEventCreateMouseEvent(source.0, 1, point, 0))
                .ok_or("Cannot create title-bar grab")?;
            let grab = Self {
                source,
                original,
                point,
                release,
            };
            CGWarpMouseCursorPosition(point);
            let moved = Owned::new(CGEventCreateMouseEvent(grab.source.0, 5, point, 0))
                .ok_or("Cannot focus window display")?;
            grab.post(moved.0, 0);
            std::thread::sleep(Duration::from_millis(50));
            grab.post(down.0, 0);
            std::thread::sleep(Duration::from_millis(50));
            let dragged = Owned::new(CGEventCreateMouseEvent(grab.source.0, 6, point, 0))
                .ok_or("Cannot begin native drag")?;
            grab.post(dragged.0, 0);
            Ok(grab)
        }
    }
    unsafe fn post(&self, event: Ref, flags: u64) {
        unsafe {
            CGEventSetFlags(event, flags);
            CGEventSetIntegerValueField(event, 42, INPUT_TAG);
            CGEventPost(0, event);
        }
    }
    unsafe fn key(&self, key: u16, flags: u64) -> Result<(), String> {
        unsafe {
            let down = Owned::new(CGEventCreateKeyboardEvent(self.source.0, key, true))
                .ok_or("Cannot create Space key")?;
            let up = Owned::new(CGEventCreateKeyboardEvent(self.source.0, key, false))
                .ok_or("Cannot create Space key release")?;
            let modifiers: Vec<_> = [(59, 1 << 18), (56, 1 << 17), (58, 1 << 19), (55, 1 << 20)]
                .into_iter()
                .filter(|(_, mask)| flags & mask != 0)
                .map(|(key, _)| {
                    let down = Owned::new(CGEventCreateKeyboardEvent(self.source.0, key, true))
                        .ok_or("Cannot create modifier")?;
                    let up = Owned::new(CGEventCreateKeyboardEvent(self.source.0, key, false))
                        .ok_or("Cannot create modifier release")?;
                    CGEventSetType(down.0, 12);
                    CGEventSetType(up.0, 12);
                    Ok((down, up))
                })
                .collect::<Result<_, &str>>()?;
            for event in [&down, &up]
                .into_iter()
                .chain(modifiers.iter().flat_map(|(down, up)| [down, up]))
            {
                CGEventSetLocation(event.0, self.point);
            }
            for (event, _) in &modifiers {
                self.post(event.0, flags);
            }
            std::thread::sleep(Duration::from_millis(20));
            self.post(down.0, flags);
            std::thread::sleep(Duration::from_millis(20));
            self.post(up.0, flags);
            for (_, event) in modifiers.iter().rev() {
                self.post(event.0, 0);
            }
            Ok(())
        }
    }
}
impl Drop for Grab {
    fn drop(&mut self) {
        unsafe {
            let current = Owned::new(CGEventCreate(null()))
                .map(|event| CGEventGetLocation(event.0))
                .unwrap_or(self.point);
            CGEventSetLocation(self.release.0, current);
            self.post(self.release.0, 0);
            // Do not fight the user if they moved the pointer during the slide.
            if (current.x - self.point.x).abs() < 2. && (current.y - self.point.y).abs() < 2. {
                CGWarpMouseCursorPosition(self.original);
            }
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
