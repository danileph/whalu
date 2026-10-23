use super::ffi::*;
use magnetic::{
    core::{Mode, Window, WindowId},
    geometry::Rect,
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSScreen, NSWorkspace};
use std::collections::{BTreeMap, BTreeSet};

pub struct NativeWindow {
    pub id: WindowId,
    pub pid: i32,
    pub element: Owned,
    pub app: Retained<NSRunningApplication>,
    pub visible: bool,
    pub quartz_id: Option<u32>,
}
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub visible: Vec<WindowId>,
    pub focused: Option<WindowId>,
    pub modal: bool,
    pub bounds: BTreeMap<u32, Rect>,
}
pub struct Discovery {
    pub handles: BTreeMap<WindowId, NativeWindow>,
    next: u64,
}
impl Discovery {
    pub fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
            next: 0,
        }
    }
    /// A click must still belong to an on-screen Quartz window at dispatch time.
    pub fn is_onscreen(&self, id: WindowId) -> bool {
        self.handles.get(&id).is_some_and(|handle| {
            handle.visible
                && handle.quartz_id.is_some_and(|cg| {
                    quartz_windows(17).is_some_and(|windows| {
                        windows.iter().any(|window| {
                            window.id == cg && window.pid == handle.pid && window.onscreen
                        })
                    })
                })
        })
    }
    pub fn scan(&mut self, mtm: MainThreadMarker, ignored: &[String]) -> Snapshot {
        let bounds = display_bounds(mtm);
        let quartz = quartz_windows(16);
        let visible: Vec<_> = quartz
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|w| w.onscreen)
            .map(|w| (w.pid, w.frame))
            .collect();
        let workspace = NSWorkspace::sharedWorkspace();
        let front = workspace
            .frontmostApplication()
            .map(|a| a.processIdentifier());
        let mut result = Snapshot {
            windows: vec![],
            visible: vec![],
            focused: None,
            modal: false,
            bounds,
        };
        let mut live = BTreeSet::new();
        let apps = workspace.runningApplications();
        let running: BTreeSet<_> = apps.iter().map(|a| a.processIdentifier()).collect();
        for handle in self.handles.values_mut() {
            handle.visible = false;
        }
        for app in apps {
            let pid = app.processIdentifier();
            if pid == std::process::id() as i32
                || app.activationPolicy() == NSApplicationActivationPolicy::Prohibited
            {
                continue;
            }
            let app_id = app
                .bundleIdentifier()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("pid:{pid}"));
            if ignored.contains(&app_id) {
                continue;
            }
            let app_name = app
                .localizedName()
                .map(|s| s.to_string())
                .unwrap_or_else(|| app_id.clone());
            let Some(ax) = (unsafe { Owned::new(AXUIElementCreateApplication(pid)) }) else {
                continue;
            };
            unsafe {
                AXUIElementSetMessagingTimeout(ax.0, 0.08);
            }
            let focused = attr(ax.0, "AXFocusedWindow");
            if front == Some(pid) {
                // A stale focused window from the old Space (or Finder's desktop)
                // must not freeze navigation in the destination Space.
                result.modal = focused.as_ref().is_some_and(|f| {
                    let known = self
                        .handles
                        .values()
                        .find(|handle| {
                            handle.pid == pid && unsafe { CFEqual(handle.element.0, f.0) }
                        })
                        .and_then(|handle| handle.quartz_id);
                    let shown = known.map_or_else(
                        || {
                            frame(f.0).is_some_and(|r| {
                                let matches: Vec<_> = quartz
                                    .as_deref()
                                    .unwrap_or_default()
                                    .iter()
                                    .filter(|window| window.pid == pid && window.frame.near(r))
                                    .collect();
                                matches.len() == 1 && matches[0].onscreen
                            })
                        },
                        |id| {
                            quartz.as_deref().unwrap_or_default().iter().any(|window| {
                                window.pid == pid && window.id == id && window.onscreen
                            })
                        },
                    );
                    shown
                        && (matches!(
                            text_attr(f.0, "AXSubrole").as_str(),
                            "AXDialog" | "AXSystemDialog"
                        ) || bool_attr(f.0, "AXModal")
                            || bool_attr(f.0, "AXFullScreen")
                            || attr(f.0, "AXSheets")
                                .is_some_and(|s| unsafe { !array(s.0).is_empty() }))
                });
            }
            let Some(windows) = attr(ax.0, "AXWindows") else {
                continue;
            };
            let elements = unsafe { array(windows.0) };
            let frames: Vec<_> = elements.iter().map(|element| frame(*element)).collect();
            for (element, window_frame) in elements.iter().copied().zip(frames.iter()) {
                if text_attr(element, "AXSubrole") != "AXStandardWindow"
                    || bool_attr(element, "AXModal")
                {
                    continue;
                }
                let Some(frame) = *window_frame else {
                    continue;
                };
                if !settable(element, "AXPosition") {
                    continue;
                }
                let candidates: Vec<_> = quartz
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|w| w.pid == pid && w.frame.near(frame))
                    .collect();
                let existing = self
                    .handles
                    .values()
                    .find(|h| h.pid == pid && unsafe { CFEqual(h.element.0, element) });
                let known = existing.and_then(|handle| handle.quartz_id).filter(|cg| {
                    quartz
                        .as_ref()
                        .is_some_and(|windows| windows.iter().any(|w| w.id == *cg && w.pid == pid))
                });
                let ax_matches = frames
                    .iter()
                    .flatten()
                    .filter(|other| other.near(frame))
                    .count();
                let quartz_id = match_quartz_window(
                    known,
                    &candidates.iter().map(|w| w.id).collect::<Vec<_>>(),
                    ax_matches,
                );
                // A refreshed AX proxy may still describe the same Quartz
                // window. Reuse that identity only if its old AX object is not
                // another window in this application's current enumeration.
                let existing = existing.or_else(|| {
                    quartz_id.and_then(|cg| {
                        self.handles.values().find(|handle| {
                            handle.pid == pid
                                && handle.quartz_id == Some(cg)
                                && !elements
                                    .iter()
                                    .any(|element| unsafe { CFEqual(handle.element.0, *element) })
                        })
                    })
                });
                let id = existing.map(|h| h.id).unwrap_or_else(|| {
                    self.next += 1;
                    self.next
                });
                let shown = !bool_attr(element, "AXMinimized")
                    && !bool_attr(element, "AXFullScreen")
                    && if let (Some(cg), Some(windows)) = (quartz_id, quartz.as_ref()) {
                        windows
                            .iter()
                            .any(|w| w.id == cg && w.pid == pid && w.onscreen)
                    } else {
                        false
                    };
                let display = result
                    .bounds
                    .iter()
                    .max_by(|(_, a), (_, b)| {
                        a.intersection(frame)
                            .area()
                            .total_cmp(&b.intersection(frame).area())
                    })
                    .map(|(id, _)| *id)
                    .unwrap_or(unsafe { CGMainDisplayID() });
                let title = text_attr(element, "AXTitle");
                result.windows.push(Window {
                    id,
                    app_id: app_id.clone(),
                    app_name: app_name.clone(),
                    title,
                    display,
                    frame,
                    mode: Mode::Magnetic,
                    last_floating_frame: None,
                    snap: None,
                    restore_frame: None,
                });
                if shown {
                    result.visible.push(id);
                }
                if front == Some(pid)
                    && focused
                        .as_ref()
                        .is_some_and(|f| unsafe { CFEqual(f.0, element) })
                    && shown
                {
                    result.focused = Some(id);
                }
                self.handles.insert(
                    id,
                    NativeWindow {
                        id,
                        pid,
                        element: unsafe { Owned(CFRetain(element)) },
                        app: app.clone(),
                        visible: shown,
                        quartz_id,
                    },
                );
                live.insert(id);
            }
        }
        // Public Quartz metadata cannot distinguish AX windows sharing identical bounds
        // on different Spaces. Exclude ambiguous matches instead of touching a hidden window.
        let ambiguous: BTreeSet<_> = result
            .visible
            .iter()
            .copied()
            .filter(|id| {
                let h = &self.handles[id];
                let r = result.windows.iter().find(|w| w.id == *id).unwrap().frame;
                let candidates = result
                    .windows
                    .iter()
                    .filter(|w| self.handles[&w.id].pid == h.pid && w.frame.near(r))
                    .count();
                let onscreen = visible
                    .iter()
                    .filter(|(pid, frame)| *pid == h.pid && frame.near(r))
                    .count();
                candidates > onscreen && result.focused != Some(*id) && h.quartz_id.is_none()
            })
            .collect();
        result.visible.retain(|id| !ambiguous.contains(id));
        for id in ambiguous {
            if let Some(h) = self.handles.get_mut(&id) {
                h.visible = false;
            }
        }
        self.handles.retain(|id, h| {
            if !running.contains(&h.pid) {
                return false;
            }
            if live.contains(id) {
                return true;
            }
            if let (Some(cg), Some(windows)) = (h.quartz_id, quartz.as_ref()) {
                return windows.iter().any(|w| w.id == cg && w.pid == h.pid);
            }
            // No reliable Quartz identity: only an explicit invalid AX object confirms
            // destruction. Timeouts/off-Space AX omissions preserve its metadata.
            unsafe {
                let mut role = std::ptr::null();
                let code =
                    AXUIElementCopyAttributeValue(h.element.0, string("AXRole").0, &mut role);
                if !role.is_null() {
                    CFRelease(role);
                }
                code != -25202
            }
        });

        result
    }
}
/// AX focus can lag a Space transition. Never use that stale focus to assign an
/// on-screen window's identity to an equally sized off-Space AX window.
fn match_quartz_window(known: Option<u32>, candidates: &[u32], ax_matches: usize) -> Option<u32> {
    known.or_else(|| (ax_matches == 1 && candidates.len() == 1).then(|| candidates[0]))
}

pub fn display_bounds(mtm: MainThreadMarker) -> BTreeMap<u32, Rect> {
    // AppKit's Y increases up; Accessibility/Quartz's Y increases down from primary top.
    let primary = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    let top = primary.origin.y + primary.size.height;
    let mut result = BTreeMap::new();
    for screen in NSScreen::screens(mtm) {
        let v = screen.visibleFrame();
        let desc = screen.deviceDescription();
        let id = unsafe { number(dict(Retained::as_ptr(&desc).cast(), "NSScreenNumber")) } as u32;
        result.insert(
            id,
            Rect::new(
                v.origin.x,
                top - v.origin.y - v.size.height,
                v.size.width,
                v.size.height,
            ),
        );
    }
    result
}
struct QuartzWindow {
    id: u32,
    pid: i32,
    frame: Rect,
    onscreen: bool,
}
fn quartz_windows(options: u32) -> Option<Vec<QuartzWindow>> {
    unsafe {
        let list = Owned::new(CGWindowListCopyWindowInfo(options, 0))?;
        Some(
            array(list.0)
                .into_iter()
                .filter_map(|d| {
                    if number(dict(d, "kCGWindowLayer")) != 0 {
                        return None;
                    }
                    let mut r = CGRect::default();
                    let bounds = dict(d, "kCGWindowBounds");
                    if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
                        return None;
                    }
                    let shown = dict(d, "kCGWindowIsOnscreen");
                    Some(QuartzWindow {
                        id: number(dict(d, "kCGWindowNumber")) as u32,
                        pid: number(dict(d, "kCGWindowOwnerPID")) as i32,
                        frame: Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height),
                        onscreen: !shown.is_null()
                            && CFGetTypeID(shown) == CFBooleanGetTypeID()
                            && CFBooleanGetValue(shown),
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::match_quartz_window;

    #[test]
    fn off_space_identity_is_not_reassigned_to_the_visible_same_size_window() {
        // 11 is off-screen, 22 is on-screen; AX may still report 11 focused.
        assert_eq!(match_quartz_window(Some(11), &[11, 22], 2), Some(11));
        assert_eq!(match_quartz_window(Some(22), &[11, 22], 2), Some(22));
        assert_eq!(match_quartz_window(Some(11), &[22], 2), Some(11));
    }
    #[test]
    fn ambiguous_new_ax_windows_do_not_share_a_visible_quartz_identity() {
        assert_eq!(match_quartz_window(None, &[22], 2), None);
        assert_eq!(match_quartz_window(None, &[11, 22], 1), None);
        assert_eq!(match_quartz_window(None, &[22], 1), Some(22));
        assert_eq!(match_quartz_window(None, &[], 1), None);
    }
}
