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
                    frame(f.0).is_some_and(|r| visible.iter().any(|(p, v)| *p == pid && v.near(r)))
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
            for element in unsafe { array(windows.0) } {
                if text_attr(element, "AXSubrole") != "AXStandardWindow"
                    || bool_attr(element, "AXModal")
                {
                    continue;
                }
                let Some(frame) = frame(element) else {
                    continue;
                };
                if !settable(element, "AXPosition") {
                    continue;
                }
                let is_focused = front == Some(pid)
                    && focused
                        .as_ref()
                        .is_some_and(|f| unsafe { CFEqual(f.0, element) });
                let candidates: Vec<_> = quartz
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|w| w.pid == pid && w.frame.near(frame))
                    .collect();
                let shown_candidates: Vec<_> = candidates.iter().filter(|w| w.onscreen).collect();
                let matched = if candidates.len() == 1 {
                    Some(candidates[0].id)
                } else if is_focused && shown_candidates.len() == 1 {
                    Some(shown_candidates[0].id)
                } else {
                    None
                };
                let existing = self
                    .handles
                    .values()
                    .find(|h| h.pid == pid && unsafe { CFEqual(h.element.0, element) })
                    .or_else(|| {
                        matched.and_then(|cg| {
                            self.handles
                                .values()
                                .find(|h| h.pid == pid && h.quartz_id == Some(cg))
                        })
                    });
                let quartz_id = matched.or_else(|| existing.and_then(|h| h.quartz_id));
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
                        visible.iter().any(|(p, r)| *p == pid && r.near(frame))
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
