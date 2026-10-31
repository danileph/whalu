use super::ffi::*;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSScreen, NSWorkspace};
use std::collections::{BTreeMap, BTreeSet};
use whalu::{
    core::{Mode, Window, WindowId},
    geometry::Rect,
};

pub struct NativeWindow {
    pub id: WindowId,
    pub pid: i32,
    pub element: Owned,
    pub app: Retained<NSRunningApplication>,
    pub visible: bool,
    pub minimized: bool,
    pub quartz_id: Option<u32>,
}
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub visible: Vec<WindowId>,
    pub focused: Option<WindowId>,
    pub modal: bool,
    pub bounds: BTreeMap<u32, Rect>,
}
impl Snapshot {
    pub fn active_display(&self, requested: Option<u32>) -> u32 {
        requested
            .filter(|display| self.bounds.contains_key(display))
            .or_else(|| {
                self.focused
                    .and_then(|id| self.windows.iter().find(|window| window.id == id))
                    .map(|window| window.display)
            })
            .or_else(|| pointer_display_among(self.bounds.keys().copied()))
            .unwrap_or(unsafe { CGMainDisplayID() })
    }

    pub fn visible_on_display(&self, display: u32) -> Vec<WindowId> {
        let local: BTreeSet<_> = self
            .windows
            .iter()
            .filter(|window| window.display == display)
            .map(|window| window.id)
            .collect();
        self.visible
            .iter()
            .copied()
            .filter(|id| local.contains(id))
            .collect()
    }
}
pub fn pointer_display(mtm: MainThreadMarker) -> Option<u32> {
    pointer_display_among(
        NSScreen::screens(mtm)
            .iter()
            .map(|screen| display_id(&screen)),
    )
}

pub fn left_edge_display(mtm: MainThreadMarker, point: Point) -> Option<u32> {
    NSScreen::screens(mtm)
        .iter()
        .map(|screen| display_id(&screen))
        .find(|display| {
            let frame = unsafe { CGDisplayBounds(*display) };
            whalu::overview::at_left_edge(
                Rect::new(
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                ),
                (point.x, point.y),
            )
        })
}

fn pointer_display_among(mut displays: impl Iterator<Item = u32>) -> Option<u32> {
    // Quartz coordinates include the full display, including menu bar and Dock.
    let event = unsafe { Owned::new(CGEventCreate(std::ptr::null())) }?;
    let point = unsafe { CGEventGetLocation(event.0) };
    displays.find(|display| {
        let frame = unsafe { CGDisplayBounds(*display) };
        point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height
    })
}
pub struct Discovery {
    pub handles: BTreeMap<WindowId, NativeWindow>,
    next: u64,
    pub(super) scan_count: u64,
    navigation_state: Option<Vec<QuartzWindow>>,
}
impl Discovery {
    pub fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
            next: 0,
            scan_count: 0,
            navigation_state: None,
        }
    }
    /// Reuse metadata only while the complete Quartz window cohort and geometry
    /// still match. Z-order changes from our own focus requests are harmless.
    pub fn navigation_focus(&self) -> Option<WindowId> {
        let previous = self.navigation_state.as_ref()?;
        let mut current = quartz_windows(16)?;
        if !navigation_unchanged(previous, &mut current) {
            return None;
        }
        let pid = NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .processIdentifier();
        let app = unsafe { Owned::new(AXUIElementCreateApplication(pid)) }?;
        unsafe { AXUIElementSetMessagingTimeout(app.0, 0.05) };
        let focused = attr(app.0, "AXFocusedWindow")?;
        let handle = self.handles.values().find(|handle| {
            handle.pid == pid && handle.visible && unsafe { CFEqual(handle.element.0, focused.0) }
        })?;
        // A known standard window can acquire a sheet or enter fullscreen
        // without changing its AX identity.
        if text_attr(focused.0, "AXSubrole") != "AXStandardWindow"
            || bool_attr(focused.0, "AXModal")
            || bool_attr(focused.0, "AXFullScreen")
            || attr(focused.0, "AXSheets").is_some_and(|s| unsafe { !array(s.0).is_empty() })
        {
            return None;
        }
        Some(handle.id)
    }

    /// Read front-to-back order while validating every requested native identity
    /// and its current monitor. Missing or moved windows invalidate the repair.
    pub fn stacking_order(
        &self,
        ids: &[WindowId],
        display: u32,
        bounds: &BTreeMap<u32, Rect>,
    ) -> Option<Vec<WindowId>> {
        let current = quartz_windows(17)?;
        let mut native = BTreeMap::new();
        for id in ids {
            let handle = self.handles.get(id)?;
            let quartz = handle.quartz_id?;
            current.iter().find(|window| {
                handle.visible
                    && window.id == quartz
                    && window.pid == handle.pid
                    && window.onscreen
                    && frame_display(window.frame, bounds) == Some(display)
            })?;
            native.insert(quartz, *id);
        }
        Some(
            current
                .iter()
                .filter_map(|window| native.get(&window.id).copied())
                .collect(),
        )
    }
    pub fn invalidate_navigation(&mut self) {
        self.navigation_state = None;
    }
    /// A click must still belong to an on-screen Quartz window at dispatch time.
    pub fn is_onscreen(&self, id: WindowId) -> bool {
        self.is_onscreen_on_display(id, None)
    }
    pub fn is_onscreen_on_display(
        &self,
        id: WindowId,
        display: Option<(u32, &BTreeMap<u32, Rect>)>,
    ) -> bool {
        let local =
            |frame| display.is_none_or(|(id, bounds)| frame_display(frame, bounds) == Some(id));
        self.handles.get(&id).is_some_and(|handle| {
            if !handle.visible {
                return false;
            }
            let Some(windows) = quartz_windows(16) else {
                return false;
            };
            if let Some(cg) = handle.quartz_id {
                return windows.iter().any(|window| {
                    window.id == cg
                        && window.pid == handle.pid
                        && window.onscreen
                        && local(window.frame)
                });
            }
            // Identical, fully on-screen windows can have distinct AX identities
            // without a reliable per-window Quartz match. Revalidate the whole
            // cohort before allowing a click on one of their icon-only cards.
            let Some(frame) = frame(handle.element.0) else {
                return false;
            };
            if !local(frame) {
                return false;
            }
            let Some(app) = (unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) })
            else {
                return false;
            };
            unsafe { AXUIElementSetMessagingTimeout(app.0, 0.05) };
            let Some(elements) = attr(app.0, "AXWindows") else {
                return false;
            };
            let elements = unsafe { array(elements.0) };
            if !elements
                .iter()
                .any(|element| unsafe { CFEqual(*element, handle.element.0) })
            {
                return false;
            }
            let ax_matches = elements
                .iter()
                .filter_map(|element| super::ffi::frame(*element))
                .filter(|other| other.near(frame))
                .count();
            let candidates: Vec<_> = windows
                .iter()
                .filter(|window| window.pid == handle.pid && window.frame.near(frame))
                .collect();
            cohort_is_onscreen(&candidates, ax_matches)
        })
    }
    pub fn scan(&mut self, mtm: MainThreadMarker, ignored: &[String]) -> Snapshot {
        self.scan_count += 1;
        let bounds = display_bounds(mtm);
        let quartz = quartz_windows(16);
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
            if ignored.contains(&app_id) || !manageable_owner(&app_id) {
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
            let focused = (front == Some(pid))
                .then(|| attr(ax.0, "AXFocusedWindow"))
                .flatten();
            if front == Some(pid) {
                // A stale focused window from the old Space (or Finder's desktop)
                // must not freeze navigation in the destination Space.
                result.modal = focused.as_ref().is_some_and(|f| {
                    let known = window_id(f.0).or_else(|| {
                        self.handles
                            .values()
                            .find(|handle| {
                                handle.pid == pid && unsafe { CFEqual(handle.element.0, f.0) }
                            })
                            .and_then(|handle| handle.quartz_id)
                    });
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
            let titles: Vec<_> = elements
                .iter()
                .map(|element| text_attr(*element, "AXTitle"))
                .collect();
            for ((element, window_frame), title) in elements
                .iter()
                .copied()
                .zip(frames.iter())
                .zip(titles.iter())
            {
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
                let direct = window_id(element);
                let quartz_id = if let Some(id) = direct {
                    verified_quartz_id(id, pid, quartz.as_deref().unwrap_or_default())
                } else {
                    match_quartz_window(
                        known,
                        &candidates,
                        &frames
                            .iter()
                            .zip(&titles)
                            .filter(|(other, _)| other.is_some_and(|other| other.near(frame)))
                            .map(|(_, title)| title.as_str())
                            .collect::<Vec<_>>(),
                        title,
                    )
                };
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
                let minimized = bool_attr(element, "AXMinimized");
                let shown = !minimized
                    && !bool_attr(element, "AXFullScreen")
                    && if let (Some(cg), Some(windows)) = (quartz_id, quartz.as_ref()) {
                        windows
                            .iter()
                            .any(|w| w.id == cg && w.pid == pid && w.onscreen)
                    } else {
                        direct.is_none() && cohort_is_onscreen(&candidates, ax_matches)
                    };
                let display =
                    frame_display(frame, &result.bounds).unwrap_or(unsafe { CGMainDisplayID() });
                result.windows.push(Window {
                    id,
                    app_id: app_id.clone(),
                    app_name: app_name.clone(),
                    title: title.clone(),
                    display,
                    frame,
                    mode: Mode::Magnetic,
                    last_floating_frame: None,
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
                        minimized,
                        quartz_id,
                    },
                );
                live.insert(id);
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

        self.navigation_state = quartz.map(|mut windows| {
            windows.sort_unstable_by_key(|window| window.id);
            windows
        });
        result
    }
}
fn frame_display(frame: Rect, bounds: &BTreeMap<u32, Rect>) -> Option<u32> {
    bounds
        .iter()
        .map(|(id, bounds)| (*id, bounds.intersection(frame).area()))
        .filter(|(_, area)| *area > 0.)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}
/// macOS exposes this permission prompt as an AXStandardWindow, although it
/// belongs to the system authorization flow and cannot be navigated normally.
fn manageable_owner(app_id: &str) -> bool {
    app_id != "com.apple.accessibility.universalAccessAuthWarn"
}
/// Never attach a capture ID to an AX window owned by a different process.
fn verified_quartz_id(id: u32, pid: i32, windows: &[QuartzWindow]) -> Option<u32> {
    windows
        .iter()
        .any(|window| window.id == id && window.pid == pid)
        .then_some(id)
}
/// AX focus can lag a Space transition. Never use that stale focus to assign an
/// on-screen window's identity to an equally sized off-Space AX window.
fn match_quartz_window(
    known: Option<u32>,
    candidates: &[&QuartzWindow],
    ax_titles: &[&str],
    title: &str,
) -> Option<u32> {
    known.or_else(|| {
        if ax_titles.len() == 1 && candidates.len() == 1 {
            return Some(candidates[0].id);
        }
        // Window names are optional (notably without Screen Recording access).
        // Only a unique title on both sides can resolve overlapping windows.
        if title.is_empty() || ax_titles.iter().filter(|other| **other == title).count() != 1 {
            return None;
        }
        let mut matches = candidates.iter().filter(|window| window.title == title);
        let matched = matches.next()?;
        matches.next().is_none().then_some(matched.id)
    })
}

fn cohort_is_onscreen(candidates: &[&QuartzWindow], ax_matches: usize) -> bool {
    ax_matches > 0
        && candidates.len() == ax_matches
        && candidates.iter().all(|window| window.onscreen)
}

pub fn display_id(screen: &NSScreen) -> u32 {
    let desc = screen.deviceDescription();
    unsafe { number(dict(Retained::as_ptr(&desc).cast(), "NSScreenNumber")) as u32 }
}

pub fn display_bounds(mtm: MainThreadMarker) -> BTreeMap<u32, Rect> {
    // AppKit's Y increases up; Accessibility/Quartz's Y increases down from primary top.
    let primary = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    let top = primary.origin.y + primary.size.height;
    let mut result = BTreeMap::new();
    for screen in NSScreen::screens(mtm) {
        let v = screen.visibleFrame();
        result.insert(
            display_id(&screen),
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
#[derive(Clone, PartialEq)]
struct QuartzWindow {
    id: u32,
    pid: i32,
    frame: Rect,
    onscreen: bool,
    title: String,
}
fn navigation_unchanged(previous: &[QuartzWindow], current: &mut [QuartzWindow]) -> bool {
    current.sort_unstable_by_key(|window| window.id);
    current == previous
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
                        title: text(dict(d, "kCGWindowName")).unwrap_or_default(),
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_limits_visible_windows_to_the_focused_display() {
        let windows = [(1, 1), (2, 2), (3, 2)]
            .into_iter()
            .map(|(id, display)| Window {
                id,
                display,
                app_id: "Editor".into(),
                app_name: "Editor".into(),
                title: format!("Document {id}"),
                frame: Rect::new(0., 30., 800., 600.),
                mode: Mode::Magnetic,
                last_floating_frame: None,
            })
            .collect();
        let mut snapshot = Snapshot {
            windows,
            visible: vec![1, 2],
            focused: Some(2),
            modal: false,
            bounds: BTreeMap::from([
                (1, Rect::new(0., 30., 1440., 850.)),
                (2, Rect::new(-1920., -200., 1920., 1080.)),
                (3, Rect::new(1440., 30., 1440., 850.)),
            ]),
        };
        assert_eq!(snapshot.active_display(None), 2);
        assert_eq!(snapshot.visible_on_display(2), [2]);
        assert_eq!(snapshot.visible_on_display(1), [1]);
        assert!(snapshot.visible_on_display(3).is_empty());
        snapshot.focused = Some(1);
        assert_eq!(snapshot.active_display(None), 1);
        // The requested monitor wins even while native focus stays elsewhere.
        assert_eq!(snapshot.active_display(Some(2)), 2);
        assert_eq!(snapshot.active_display(Some(3)), 3);
        // A disconnected requested monitor falls back to the focused display.
        snapshot.bounds.remove(&3);
        assert_eq!(snapshot.active_display(Some(3)), 1);
    }

    #[test]
    fn display_membership_tracks_majority_area_and_rejects_removed_displays() {
        let bounds = BTreeMap::from([
            (1, Rect::new(0., 30., 1440., 850.)),
            (2, Rect::new(-1920., -200., 1920., 1080.)),
        ]);
        assert_eq!(
            frame_display(Rect::new(-800., 50., 600., 600.), &bounds),
            Some(2)
        );
        assert_eq!(
            frame_display(Rect::new(-100., 50., 600., 600.), &bounds),
            Some(1)
        );
        assert_eq!(
            frame_display(Rect::new(-500., 50., 600., 600.), &bounds),
            Some(2)
        );
        let remaining = BTreeMap::from([(1, bounds[&1])]);
        assert_eq!(
            frame_display(Rect::new(-800., 50., 600., 600.), &remaining),
            None
        );
    }

    #[test]
    fn accessibility_authorization_helper_is_never_an_application_window_owner() {
        assert!(!manageable_owner(
            "com.apple.accessibility.universalAccessAuthWarn"
        ));
        // Do not filter real applications or third-party accessory windows.
        for app in [
            "com.apple.systempreferences",
            "com.apple.finder",
            "com.openai.codex",
            "org.hammerspoon.Hammerspoon",
        ] {
            assert!(manageable_owner(app));
        }
    }

    fn quartz(id: u32, title: &str, onscreen: bool) -> QuartzWindow {
        QuartzWindow {
            id,
            pid: 7,
            frame: Rect::new(0., 30., 800., 600.),
            onscreen,
            title: title.into(),
        }
    }

    #[test]
    fn overlapping_windows_match_their_own_unique_titles() {
        let first = quartz(11, "First document", true);
        let second = quartz(22, "Second document", true);
        let candidates = [&second, &first];
        let titles = ["First document", "Second document"];
        assert_eq!(
            match_quartz_window(None, &candidates, &titles, titles[0]),
            Some(11)
        );
        assert_eq!(
            match_quartz_window(None, &candidates, &titles, titles[1]),
            Some(22)
        );
    }

    #[test]
    fn native_capture_ids_distinguish_identical_windows_and_validate_the_owner() {
        let first = quartz(11, "Recents", true);
        let second = quartz(22, "Recents", false);
        let windows = [first, second];
        assert_eq!(verified_quartz_id(11, 7, &windows), Some(11));
        assert_eq!(verified_quartz_id(22, 7, &windows), Some(22));
        // A stale or cross-process ID must not fall back to its identical peer.
        assert_eq!(verified_quartz_id(33, 7, &windows), None);
        assert_eq!(verified_quartz_id(11, 8, &windows), None);
    }

    #[test]
    fn off_space_identity_is_not_reassigned_to_the_visible_same_size_window() {
        let hidden = quartz(11, "Document", false);
        let shown = quartz(22, "Document", true);
        let titles = ["Document", "Document"];
        assert_eq!(
            match_quartz_window(Some(11), &[&hidden, &shown], &titles, titles[0]),
            Some(11)
        );
        assert_eq!(
            match_quartz_window(Some(22), &[&hidden, &shown], &titles, titles[1]),
            Some(22)
        );
        assert_eq!(
            match_quartz_window(Some(11), &[&shown], &titles, titles[0]),
            Some(11)
        );
        assert!(!cohort_is_onscreen(&[&hidden, &shown], 2));
        assert!(!cohort_is_onscreen(&[&shown], 2));
    }

    #[test]
    fn ambiguous_new_ax_windows_do_not_share_a_visible_quartz_identity() {
        let first = quartz(11, "Document", true);
        let second = quartz(22, "Document", true);
        assert_eq!(
            match_quartz_window(None, &[&second], &["Document", "Document"], "Document"),
            None
        );
        assert_eq!(
            match_quartz_window(None, &[&first, &second], &["Document"], "Document"),
            None
        );
        assert_eq!(
            match_quartz_window(None, &[&second], &["Document"], "Document"),
            Some(22)
        );
        assert_eq!(
            match_quartz_window(None, &[], &["Document"], "Document"),
            None
        );
    }

    #[test]
    fn indistinguishable_windows_are_visible_only_when_the_complete_cohort_is_onscreen() {
        let first = quartz(11, "", true);
        let second = quartz(22, "", true);
        let candidates = [&first, &second];
        assert_eq!(match_quartz_window(None, &candidates, &["", ""], ""), None);
        assert!(cohort_is_onscreen(&candidates, 2));
        assert!(!cohort_is_onscreen(&candidates, 1));
        assert!(!cohort_is_onscreen(&candidates, 3));
        assert!(!cohort_is_onscreen(&[], 0));
    }

    #[test]
    fn unique_titles_resolve_windows_on_separate_spaces() {
        let hidden = quartz(11, "Hidden", false);
        let shown = quartz(22, "Visible", true);
        let candidates = [&hidden, &shown];
        let titles = ["Hidden", "Visible"];
        assert_eq!(
            match_quartz_window(None, &candidates, &titles, titles[0]),
            Some(11)
        );
        assert_eq!(
            match_quartz_window(None, &candidates, &titles, titles[1]),
            Some(22)
        );
        assert!(!cohort_is_onscreen(&candidates, 2));
    }
}

#[cfg(test)]
mod navigation_cache_tests {
    use super::*;

    #[test]
    fn cache_accepts_focus_reordering_but_rejects_space_and_geometry_changes() {
        let previous = vec![
            QuartzWindow {
                id: 11,
                pid: 7,
                frame: Rect::new(-800., 0., 800., 600.),
                onscreen: true,
                title: String::new(),
            },
            QuartzWindow {
                id: 22,
                pid: 7,
                frame: Rect::new(-800., 0., 800., 600.),
                onscreen: false,
                title: String::new(),
            },
        ];
        let mut reordered = previous.clone();
        reordered.reverse();
        assert!(navigation_unchanged(&previous, &mut reordered));

        // Equally sized windows on separate Spaces cannot share cached visibility.
        let mut switched = previous.clone();
        switched[0].onscreen = false;
        switched[1].onscreen = true;
        assert!(!navigation_unchanged(&previous, &mut switched));

        let mut moved = previous.clone();
        moved[0].frame.x += 20.;
        assert!(!navigation_unchanged(&previous, &mut moved));
        let mut replaced = previous.clone();
        replaced[0].id = 33;
        assert!(!navigation_unchanged(&previous, &mut replaced));
        let mut restarted = previous.clone();
        restarted[0].pid = 8;
        assert!(!navigation_unchanged(&previous, &mut restarted));
        assert!(!navigation_unchanged(
            &previous,
            &mut previous[..1].to_vec()
        ));
    }
}
