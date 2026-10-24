//! WindowServer Space transfer; no custom window positioning or resizing.
//! Dynamically resolved because macOS exposes no public window-to-Space API.
//! Uses the asynchronous native window-management bridge; verified membership,
//! rather than an undocumented return value, establishes success.
//! https://gist.github.com/stephancasas-openai/1b31a8d76c6a103e4676ac196e06e9d8
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

pub fn move_adjacent(window: u32, forward: bool) -> Result<bool, String> {
    unsafe {
        let connection: Connection = symbol(c"SLSMainConnectionID")?;
        let displays: Displays = symbol(c"SLSCopyManagedDisplaySpaces")?;
        let window_spaces: WindowSpaces = symbol(c"SLSCopySpacesForWindows")?;
        let connection = connection();
        let window_number = i64::from(window);
        let number = Owned::new(CFNumberCreate(
            null(),
            4,
            (&window_number as *const i64).cast(),
        ))
        .ok_or("Cannot create window identity")?;
        // Both retained values outlive every WindowServer query.
        let windows = Owned::new(CFArrayCreate(null(), &number.0, 1, null()))
            .ok_or("Cannot create window list")?;
        let memberships = Owned::new(window_spaces(connection, 7, windows.0))
            .ok_or("Cannot read window Spaces")?;
        let memberships = array(memberships.0);
        // Sticky/all-desktop windows and fullscreen Spaces are not transferable.
        if memberships.len() != 1 {
            return Ok(false);
        }
        let source = super::ffi::number(memberships[0]) as u64;
        let managed = Owned::new(displays(connection)).ok_or("Cannot read desktop order")?;
        for display in array(managed.0) {
            let desktops: Vec<_> = array(dict(display, "Spaces"))
                .into_iter()
                .filter(|space| super::ffi::number(dict(*space, "type")) == 0)
                .map(|space| super::ffi::number(dict(space, "ManagedSpaceID")) as u64)
                .filter(|id| *id != 0)
                .collect();
            let Some(index) = desktops.iter().position(|id| *id == source) else {
                continue;
            };
            let Some(target) = adjacent(&desktops, index, forward) else {
                return Ok(false);
            };
            let class = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation")
                .ok_or("Native Space transfer is unavailable on this macOS")?;
            let allocated: *mut AnyObject = msg_send![class, alloc];
            let operation: *mut AnyObject = msg_send![allocated,
                initWithWindows: &*(windows.0 as *const AnyObject), spaceID: target];
            let operation =
                Retained::from_raw(operation).ok_or("Cannot create native Space transfer")?;
            let supported: bool =
                msg_send![&operation, respondsToSelector: sel!(performWithWMBridgeDelegate)];
            if !supported {
                return Err("Native Space transfer bridge is unavailable".into());
            }
            let _: () = msg_send![&operation, performWithWMBridgeDelegate];
            // The bridge is asynchronous. WindowServer membership is read back
            // without pumping AppKit's run loop (which would reenter Runtime).
            let deadline = Instant::now() + Duration::from_millis(600);
            loop {
                let actual = Owned::new(window_spaces(connection, 7, windows.0))
                    .ok_or("Cannot verify destination desktop")?;
                if array(actual.0)
                    .iter()
                    .any(|id| super::ffi::number(*id) as u64 == target)
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("WindowServer did not move the window".into());
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            return Ok(true);
        }
        Ok(false)
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
