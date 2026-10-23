//! Read-only native Space membership. No Space/window mutation SPI is loaded.
//! ABI reference: https://github.com/Hammerspoon/hammerspoon/blob/master/extensions/spaces/libspaces.m
use super::ffi::*;
use std::{collections::BTreeSet, ffi::c_void, ptr::null, sync::OnceLock};

type Connection = unsafe extern "C" fn() -> i32;
type Displays = unsafe extern "C" fn(i32) -> Ref;
type Windows = unsafe extern "C" fn(i32, u32, Ref, u32, *mut u64, *mut u64) -> Ref;
type Animating = unsafe extern "C" fn(i32, Ref) -> bool;

struct Api {
    connection: Connection,
    displays: Displays,
    windows: Windows,
    animating: Animating,
}
impl Api {
    fn get() -> Option<&'static Self> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(|| unsafe {
            let library = libc::dlopen(
                c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
                libc::RTLD_LAZY | libc::RTLD_LOCAL,
            );
            if library.is_null() {
                return None;
            }
            let connection = libc::dlsym(library, c"SLSMainConnectionID".as_ptr());
            let displays = libc::dlsym(library, c"SLSCopyManagedDisplaySpaces".as_ptr());
            let windows = libc::dlsym(library, c"SLSCopyWindowsWithOptionsAndTags".as_ptr());
            let animating = libc::dlsym(library, c"SLSManagedDisplayIsAnimating".as_ptr());
            if [connection, displays, windows, animating]
                .iter()
                .any(|symbol| symbol.is_null())
            {
                return None;
            }
            // Keep the library loaded for the lifetime of these function pointers.
            Some(Self {
                connection: std::mem::transmute::<*mut c_void, Connection>(connection),
                displays: std::mem::transmute::<*mut c_void, Displays>(displays),
                windows: std::mem::transmute::<*mut c_void, Windows>(windows),
                animating: std::mem::transmute::<*mut c_void, Animating>(animating),
            })
        })
        .as_ref()
    }
}

#[derive(Default)]
pub struct SpaceVisibility {
    windows: BTreeSet<u32>,
    animating: bool,
}
impl SpaceVisibility {
    /// None means the SPI is unavailable. A failed/incomplete native query instead
    /// returns an empty scope, preventing writes based on stale Quartz visibility.
    pub fn read() -> Option<Self> {
        let api = Api::get()?;
        Some(unsafe { Self::query(api) }.unwrap_or_default())
    }

    unsafe fn query(api: &Api) -> Option<Self> {
        let connection = unsafe { (api.connection)() };
        let displays = unsafe { Owned::new((api.displays)(connection)) }?;
        let mut current = vec![];
        let mut animating = false;
        for display in unsafe { array(displays.0) } {
            let identifier = unsafe { dict(display, "Display Identifier") };
            let space = unsafe { dict(display, "Current Space") };
            if identifier.is_null() || space.is_null() {
                return None;
            }
            animating |= unsafe { (api.animating)(connection, identifier) };
            let id = unsafe { dict(space, "id64") };
            if id.is_null() || unsafe { number(id) } == 0 {
                return None;
            }
            current.push(id);
        }
        if current.is_empty() || animating {
            return Some(Self {
                animating,
                ..Self::default()
            });
        }
        // Borrow the CFNumbers from `displays`, which stays alive through the call.
        let spaces = unsafe {
            Owned::new(CFArrayCreate(
                null(),
                current.as_ptr(),
                current.len() as isize,
                null(),
            ))
        }?;
        let mut set_tags = 0;
        let mut clear_tags = 0;
        let windows = unsafe {
            Owned::new((api.windows)(
                connection,
                0,
                spaces.0,
                0x2,
                &mut set_tags,
                &mut clear_tags,
            ))
        }?;
        Some(Self {
            windows: unsafe { array(windows.0) }
                .into_iter()
                .map(|value| unsafe { number(value) } as u32)
                .collect(),
            animating,
        })
    }

    pub fn allows(&self, window: u32) -> bool {
        !self.animating && self.windows.contains(&window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_onscreen_windows_from_the_previous_space_are_excluded() {
        let destination = SpaceVisibility {
            windows: BTreeSet::from([22, 33]),
            animating: false,
        };
        assert!(!destination.allows(11)); // Finder still reported on-screen by Quartz.
        assert!(destination.allows(22));
        assert!(destination.allows(33)); // A sticky window legitimately in this Space.
        assert!(!SpaceVisibility::default().allows(11)); // Empty destination desktop.
    }

    #[test]
    fn animation_blocks_even_windows_reported_in_the_active_space() {
        let transition = SpaceVisibility {
            windows: BTreeSet::from([11, 22]),
            animating: true,
        };
        assert!(!transition.allows(11));
        assert!(!transition.allows(22));
    }
}
