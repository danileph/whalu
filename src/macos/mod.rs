//! All native callbacks enqueue work; Accessibility calls never run in the event tap.
#![allow(unsafe_op_in_unsafe_fn)]
mod discovery;
mod ffi;
mod input_probe;
mod onboarding;
mod previews;
pub mod probe;
mod spaces;
mod tiling;
mod trackpad;
mod ui;
use discovery::Discovery;
use ffi::*;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy, NSWorkspace,
};
use objc2_foundation::{NSNotificationCenter, NSString};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    ptr::{null, null_mut},
    sync::{
        Mutex,
        atomic::{AtomicPtr, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};
use whalu::{
    config::{Config, config_dir},
    core::{Command, Effect, Engine},
    gestures::{Axis, GestureEvent},
    input::{KeyRoute, Modifiers, ShortcutRouter},
    navigation::{ClickSequence, ScrubSession},
    persistence::{SavedState, StateWriter},
    scroll::{KineticScroll, WheelRoute, WheelRouter, WheelScroll},
    visibility::DisplayContexts,
};

static QUEUE: AtomicPtr<Queue> = AtomicPtr::new(null_mut());
const OVERVIEW_FEEDBACK: Duration = Duration::from_millis(650);
// Protect dispatch lifetime for trackpad callbacks arriving on a framework thread.
static DISPATCH: Mutex<Option<(Sender<Event>, usize)>> = Mutex::new(None);
#[derive(Debug)]
pub(super) enum Event {
    Dirty,
    FocusChanged,
    DisplaysChanged,
    SpaceChanged,
    OverviewRefresh,
    SpaceTransferFinished,
    SpaceTransferPreparing(u64),
    NativeGesture,
    Command(Command),
    WindowSwitch(Command),
    OverviewHeld(bool),
    PointerMoved {
        point: Point,
        dragging: bool,
    },
    Mouse(bool),
    SelectThumbnail {
        context: u64,
        window: u64,
        epoch: u64,
    },
    ThumbnailPointer {
        context: u64,
        window: u64,
        epoch: u64,
        kind: u32,
        point: objc2_foundation::NSPoint,
    },
    Drag,
    Gesture(usize, GestureEvent, f64),
    GestureMotion(usize, f64, f64, bool),
    ContactBegan,
    TouchOverview(usize, bool),
    Wheel(f64, f64),
    Settings,
    PreviewAccess,
    PreviewReady(u64, u32, Option<previews::Image>),
    Reload,
    Quit,
    TapDisabled,
}
struct Queue {
    tx: Sender<Event>,
    source: Ref,
    bindings: Vec<whalu::config::Binding>,
    shortcuts: ShortcutRouter,
    runtime: *mut Runtime,
    scroll_modifiers: Modifiers,
    overview_held: bool,
    wheel: WheelRouter,
    clock: Instant,
    clicks: ui::ThumbnailClicks,
}
impl Queue {
    fn scroll_held(&mut self, modifiers: Modifiers, point: objc2_foundation::NSPoint) -> bool {
        // Hover owns wheel navigation without creating a keyboard hold. Read
        // the rendered hit map; never call AppKit or Accessibility in the tap.
        self.update_overview_hold(modifiers) || self.clicks.contains_pointer(point)
    }
    fn update_overview_hold(&mut self, modifiers: Modifiers) -> bool {
        let held = self
            .shortcuts
            .overview_held(modifiers, self.scroll_modifiers);
        if held != self.overview_held {
            self.overview_held = held;
            enqueue(Event::OverviewHeld(held));
        }
        held
    }
}
pub(super) fn enqueue(event: Event) {
    if let Ok(dispatch) = DISPATCH.lock() {
        if let Some((tx, source)) = dispatch.as_ref() {
            unsafe {
                let _ = tx.send(event);
                CFRunLoopSourceSignal(*source as Ref);
                CFRunLoopWakeUp(CFRunLoopGetMain());
            }
        }
    }
}
struct Observer {
    value: Owned,
    source: Ref,
    registered: BTreeSet<u64>,
}
impl Drop for Observer {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), self.source, kCFRunLoopCommonModes);
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrollSource {
    Trackpad(usize),
    Wheel,
}
// A destination-Space swipe can finish before the native transition guard.
// Keep its latest intent, never a list of old-Space window IDs or focus effects.
struct PendingSwipe {
    device: usize,
    axis: Axis,
    display: u32,
    space: u64,
    offset: i32,
    position: f64,
    time: f64,
    released: bool,
    poll: Instant,
}
struct ActiveScroll {
    source: ScrollSource,
    session: ScrubSession,
    motion: KineticScroll,
    wheel: WheelScroll,
    position: f64,
    last_time: f64,
    last_input: Instant,
    released: bool,
    changed: bool,
    sustained: bool,
}
struct ClickTravel {
    sequence: ClickSequence,
    due: Instant,
    background: Option<BackgroundOrder>,
}
struct BackgroundOrder {
    windows: Vec<u64>,
    identities: Vec<(u64, i32, u32)>,
    display: u32,
    space: u64,
    epoch: u64,
    raised: usize,
    waiting: bool,
    confirmed: bool,
    until: Instant,
}
struct PendingFocus {
    context: u64,
    window: u64,
    due: Instant,
    until: Instant,
}
impl PendingFocus {
    fn observed(
        &self,
        context: u64,
        visible: &[u64],
        observed: Option<u64>,
        now: Instant,
    ) -> Option<u64> {
        if self.context == context && visible.contains(&self.window) && now < self.until {
            Some(self.window)
        } else {
            observed
        }
    }
}
struct Runtime {
    mtm: MainThreadMarker,
    rx: Receiver<Event>,
    config: Config,
    config_path: std::path::PathBuf,
    state_writer: Option<StateWriter>,
    engine: Engine,
    discovery: Discovery,
    ui: ui::Ui,
    observers: BTreeMap<i32, Observer>,
    tap: Owned,
    gestures: Option<trackpad::Monitor>,
    gesture: Option<ActiveScroll>,
    pending_swipe: Option<PendingSwipe>,
    click_travel: Option<ClickTravel>,
    pending_focus: Option<PendingFocus>,
    motion_due: Option<Instant>,
    linger_until: Option<Instant>,
    motion_tick: Instant,
    _tap_source: Owned,
    timer: Option<Owned>,
    deadline: Option<Instant>,
    overlay_deadline: Option<Instant>,
    held: bool,
    edge_display: Option<u32>,
    touch_overview: Option<usize>,
    switch_feedback: bool,
    mouse: bool,
    dragging: bool,
    manual_until: Option<Instant>,
    switched_space: bool,
    pending_commands: Vec<Command>,
    space_until: Option<Instant>,
    space_retries: u8,
    space_transfer_display: Option<u32>,
    space_transfer_target: Option<u64>,
    space_transfer_window: Option<u64>,
    space_overview: bool,
    overview_space: Option<(u32, u64)>,
    overview_refresh_pending: Cell<bool>,
    modal: bool,
    recovery: SavedState,
    restored: BTreeSet<u64>,
    context_tracker: DisplayContexts,
    // Keep the invocation's monitor through AX activation and image refreshes.
    navigation_display: Option<u32>,
    _lock: std::fs::File,
}
impl Runtime {
    fn overview_held(&self) -> bool {
        self.held || self.edge_display.is_some() || self.touch_overview.is_some()
    }

    fn pointer_moved(&mut self, point: Point, dragging: bool) {
        if self.mouse {
            return;
        }
        let eligible = self.config.overlay && !dragging && !self.mouse;
        let overview = if eligible && self.ui.contains_pointer(point) {
            self.engine.display()
        } else {
            None
        };
        let display = if eligible {
            discovery::left_edge_display(self.mtm, point).or(overview)
        } else {
            None
        };
        if display == self.edge_display {
            return;
        }
        self.edge_display = display;
        if let Some(display) = display {
            if overview != Some(display) {
                self.clear_scroll();
                self.pending_focus = None;
                self.overlay_deadline = None;
                self.navigation_display = Some(display);
                // Opening is read-only: refresh this monitor without focusing it.
                self.reconcile(false);
                self.ui.preload_previews(&self.discovery);
            }
            // Entering visible cards only takes ownership of dismissal. It
            // must not restart navigation, clear a click, or perform a census.
            self.linger_until = None;
        } else {
            self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        }
        self.render();
    }

    fn schedule(&mut self) {
        unsafe {
            if let Some(t) = self.timer.take() {
                CFRunLoopTimerInvalidate(t.0);
            }
            let scan_due = if self.space_until.is_none()
                || (self.space_overview && (self.overview_held() || self.pending_swipe.is_some()))
            {
                self.deadline
            } else {
                None
            };
            let due = [
                scan_due,
                self.overlay_deadline,
                self.space_until,
                self.pending_swipe.as_ref().map(|p| p.poll),
                self.motion_due,
                self.linger_until,
                self.click_travel.as_ref().map(|travel| travel.due),
                self.ui.drag_due(),
                self.pending_focus.as_ref().map(|focus| focus.due),
            ]
            .into_iter()
            .flatten()
            .min();
            let Some(due) = due else {
                return;
            };
            let mut ctx = TimerContext {
                version: 0,
                info: null_mut(),
                retain: None,
                release: None,
                description: None,
            };
            let timer = Owned(CFRunLoopTimerCreate(
                null(),
                CFAbsoluteTimeGetCurrent()
                    + due.saturating_duration_since(Instant::now()).as_secs_f64(),
                0.,
                0,
                0,
                timer_callback,
                &mut ctx,
            ));
            CFRunLoopAddTimer(CFRunLoopGetMain(), timer.0, kCFRunLoopCommonModes);
            self.timer = Some(timer);
        }
    }
    fn drain(&mut self) {
        let mut focus_changed = false;
        while let Ok(event) = self.rx.try_recv() {
            let window_switch = matches!(event, Event::WindowSwitch(_));
            match event {
                Event::Dirty => self.deadline = Some(Instant::now() + Duration::from_millis(70)),
                Event::FocusChanged => focus_changed = true,
                Event::DisplaysChanged => {
                    // Rebuild display geometry without treating a monitor change
                    // as a new Space or retaining clicks in the old coordinates.
                    self.clear_scroll();
                    self.discovery.invalidate_navigation();
                    self.pending_focus = None;
                    self.ui.reset_overview();
                    self.navigation_display = None;
                    if let Some(event) = unsafe { Owned::new(CGEventCreate(null())) } {
                        self.pointer_moved(unsafe { CGEventGetLocation(event.0) }, self.dragging);
                    }
                    if self.overview_held() {
                        self.use_pointer_display();
                    }
                    self.deadline = Some(Instant::now() + Duration::from_millis(70));
                }
                Event::OverviewRefresh => {
                    self.overview_refresh_pending.set(false);
                    self.deadline = None;
                    self.discovery.invalidate_navigation();
                    self.reconcile(false);
                }
                Event::SpaceChanged => {
                    if self.space_transfer_display.is_some() {
                        // The worker owns settlement and focus. Preserve user
                        // commands queued during this particular transfer.
                        self.switched_space = true;
                        self.discovery.invalidate_navigation();
                        if self.space_transfer_target.is_some() {
                            self.stage_space_destination();
                        }
                    } else {
                        self.yield_to_system(true);
                    }
                }
                Event::SpaceTransferPreparing(target) => {
                    if self.space_transfer_display.is_some() {
                        self.space_transfer_target = Some(target);
                        if self.overview_held() && self.config.overlay {
                            self.ui.preload_previews(&self.discovery);
                        }
                        self.stage_space_destination();
                    }
                }
                Event::SpaceTransferFinished => {
                    if self.space_transfer_display.is_some() {
                        self.space_until = Some(Instant::now());
                    }
                }
                Event::NativeGesture => self.yield_to_system(false),
                Event::PointerMoved { point, dragging } => self.pointer_moved(point, dragging),
                Event::OverviewHeld(held) => {
                    let opening = held && !self.held && self.config.overlay;
                    if opening {
                        self.overlay_deadline = Some(
                            Instant::now() + Duration::from_millis(self.config.overlay_delay_ms),
                        );
                    }
                    if !held {
                        self.overlay_deadline = None;
                        if self.held && self.switch_feedback {
                            self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
                            self.switch_feedback = false;
                        }
                    }
                    self.held = held;
                    if opening {
                        self.use_pointer_display();
                    }
                    if opening && !self.mouse {
                        self.reconcile(false);
                    }
                    self.render();
                    if opening {
                        self.ui.preload_previews(&self.discovery);
                    }
                }
                Event::Gesture(device, event, time) => self.gesture_event(device, event, time),
                Event::GestureMotion(device, position, time, sustained) => {
                    if let Some(pending) =
                        self.pending_swipe.as_mut().filter(|p| p.device == device)
                    {
                        pending.position = position;
                        pending.time = time;
                    }
                    if let Some(active) = self.gesture.as_mut().filter(|active| {
                        active.source == ScrollSource::Trackpad(device) && !active.released
                    }) {
                        active.sustained = sustained;
                        active.motion.sample(position, time);
                        active.position = position;
                        active.last_time = time;
                        active.last_input = Instant::now();
                    }
                }
                Event::ContactBegan => self.contact_began(),
                Event::TouchOverview(device, held) => self.touch_overview(device, held),
                Event::Wheel(delta, time) => self.wheel_event(delta, time),
                Event::SelectThumbnail {
                    context,
                    window,
                    epoch,
                } => {
                    if epoch == self.ui.visibility_epoch() {
                        self.ui.cancel_drag();
                        self.select_thumbnail(context, window);
                    }
                }
                Event::ThumbnailPointer {
                    context,
                    window,
                    epoch,
                    kind,
                    point,
                } => {
                    if epoch != self.ui.visibility_epoch()
                        || context != self.engine.current
                        || !self.ui.is_visible()
                        || self.engine.frozen
                        || self.space_until.is_some()
                    {
                        continue;
                    }
                    if kind == 1 {
                        if !self.discovery.is_onscreen_on_display(
                            window,
                            self.engine
                                .display()
                                .map(|display| (display, &self.engine.bounds)),
                        ) {
                            self.ui.reset_overview();
                            self.deadline = Some(Instant::now());
                            continue;
                        }
                        self.clear_scroll();
                        self.pending_focus = None;
                        self.overlay_deadline = None;
                    }
                    let destination = self.ui.thumbnail_pointer(&self.engine, window, kind, point);
                    if kind == 2 {
                        if let Some(destination) = destination {
                            self.engine.reorder(window, destination);
                            self.save();
                        }
                        self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
                    }
                    self.render();
                }
                Event::Drag => self.dragging = true,
                Event::Mouse(down) => {
                    self.mouse = down;
                    self.engine.frozen = down || self.modal || self.space_until.is_some();
                    if down {
                        self.touch_overview = None;
                        self.edge_display = None;
                        self.navigation_display = None;
                        self.pending_focus = None;
                        self.overlay_deadline = None;
                        self.clear_scroll();
                        self.dragging = false;
                    }
                    if !down && self.dragging {
                        self.manual_until = Some(Instant::now() + Duration::from_millis(350));
                        self.dragging = false;
                    }
                    if !down {
                        self.deadline = Some(Instant::now() + Duration::from_millis(90));
                    }
                    self.render();
                }
                Event::Command(cmd) | Event::WindowSwitch(cmd) => {
                    self.switch_feedback |= window_switch;
                    if self.space_until.is_some() {
                        if self.pending_commands.len() < 32 {
                            self.pending_commands.push(cmd);
                        }
                        continue;
                    }
                    if self.mouse || self.ui.pointer_active() {
                        continue;
                    }
                    // Explicit keyboard input takes over from a scroll or its coast.
                    self.clear_scroll();
                    // Navigation is explicit intent: bypass the modifier-only HUD delay.
                    self.overlay_deadline = None;
                    self.prepare_command(cmd);
                    if std::env::var_os("WHALU_TRACE").is_some() {
                        eprintln!(
                            "Command {cmd:?}: context={}, windows={}, focused={:?}",
                            self.engine.current,
                            self.engine.context().map_or(0, |c| c.order.len()),
                            self.engine.focused()
                        );
                    }
                    if let Command::MoveSpace(forward) = cmd {
                        if !self.engine.frozen {
                            if let Some(handle) = self
                                .engine
                                .focused()
                                .and_then(|id| self.discovery.handles.get(&id))
                            {
                                if let Some(window) = handle.quartz_id {
                                    let submitted = tiling::submit_space(
                                        handle.pid,
                                        handle.element.0,
                                        window,
                                        forward,
                                    );
                                    match submitted {
                                        Ok(()) => {
                                            self.space_transfer_window = self.engine.focused();
                                            self.space_transfer_display = self.engine.display();
                                            self.navigation_display = self.engine.display();
                                            self.yield_to_system(true);
                                            // Bridge completion ends this early; bound a lost
                                            // completion without replaying stale focus work.
                                            self.space_until =
                                                Some(Instant::now() + Duration::from_secs(4));
                                        }
                                        Err(error) => eprintln!("Space transfer: {error}"),
                                    }
                                }
                            }
                        }
                    }
                    if let Command::Native(shortcut) = cmd {
                        if !self.engine.frozen {
                            if let Some(handle) = self
                                .engine
                                .focused()
                                .and_then(|id| self.discovery.handles.get(&id))
                            {
                                if let Err(error) =
                                    tiling::submit(handle.pid, handle.element.0, shortcut)
                                {
                                    eprintln!("Native tiling: {error}");
                                }
                            }
                        }
                    }
                    let effects = self.engine.command(cmd);
                    self.apply(effects);
                    self.render();
                    self.save();
                }

                Event::PreviewAccess => previews::request_access(),
                Event::PreviewReady(id, quartz, image) => {
                    let retry = self.ui.preview_ready(id, quartz, image);
                    self.render();
                    if retry && self.overview_held() {
                        self.deadline = Some(Instant::now() + Duration::from_millis(130));
                    }
                }
                Event::Settings => {
                    let path = self.config_path.clone();
                    if !path.exists() {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(s) = toml::to_string_pretty(&self.config) {
                            let _ = std::fs::write(&path, s);
                        }
                    }
                    let _ = std::process::Command::new("/usr/bin/open")
                        .arg("-t")
                        .arg(path)
                        .spawn();
                }
                Event::Reload => self.reload(),
                Event::Quit => {
                    self.save();
                    // Join the writer only at shutdown so the final state is durable.
                    self.state_writer.take();
                    self.ui.hide();
                    NSApplication::sharedApplication(self.mtm).terminate(None);
                }
                Event::TapDisabled => {
                    self.touch_overview = None;
                    self.clear_scroll();
                    unsafe {
                        let q = QUEUE.load(Ordering::Acquire);
                        if !q.is_null() {
                            (*q).wheel.reset();
                            (*q).clicks.reset();
                            (*q).overview_held = false;
                        }
                    }
                    self.held = false;
                    self.switch_feedback = false;
                    self.overlay_deadline = None;
                    self.render();
                    unsafe {
                        CGEventTapEnable(self.tap.0, true);
                    }
                }
            }
        }
        if focus_changed && self.space_until.is_none() && !self.mouse {
            let before = self.engine.focused();
            if self.modal || !self.refresh_navigation_focus() {
                self.deadline = Some(Instant::now() + Duration::from_millis(70));
            } else if self.engine.focused() != before {
                self.render();
            }
        }
        let now = Instant::now();
        if let Some(pending) = self.pending_swipe.as_mut().filter(|p| p.poll <= now) {
            if spaces::active_space(pending.display) != Some(pending.space) {
                self.clear_scroll();
                self.render();
            } else if spaces::is_animating(pending.display) == Some(false) {
                // A fresh destination gesture need not wait out the fallback
                // timeout once Dock has actually finished its Space animation.
                self.space_until = Some(now);
            } else {
                pending.poll = now + Duration::from_millis(16);
            }
        }
        if self.space_until.is_some_and(|d| d <= now) {
            self.space_until = None;
            self.deadline = None;
            if self.overview_held() && self.navigation_display.is_none() {
                self.use_pointer_display();
            }
            self.reconcile(false);
            self.space_transfer_display = None;
            self.space_transfer_target = None;
            self.space_transfer_window = None;
            self.space_overview = false;
            self.resume_pending_swipe();
            for cmd in std::mem::take(&mut self.pending_commands) {
                enqueue(Event::Command(cmd));
            }
        } else if (self.space_until.is_none()
            || (self.space_overview && (self.overview_held() || self.pending_swipe.is_some())))
            && self.deadline.is_some_and(|d| d <= now)
        {
            self.deadline = None;
            self.reconcile(true);
        }
        if self.overlay_deadline.is_some_and(|d| d <= now) {
            self.overlay_deadline = None;
            self.render();
        }
        self.advance_scroll(now);
        self.advance_click(now);
        if self.ui.advance_drag(now) {
            self.render();
        }
        self.confirm_focus(now);
        if !self.overview_held()
            && self.gesture.is_none()
            && self.pending_swipe.is_none()
            && self.click_travel.is_none()
            && self.linger_until.is_none()
            && !self.ui.pointer_active()
        {
            self.navigation_display = None;
        }
        self.schedule();
    }
    fn native_focus_is(&self, window: u64) -> bool {
        self.discovery.handles.get(&window).is_some_and(|handle| {
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|app| app.processIdentifier() == handle.pid)
                && unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
                    .and_then(|app| {
                        unsafe { AXUIElementSetMessagingTimeout(app.0, 0.05) };
                        attr(app.0, "AXFocusedWindow")
                    })
                    .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
        })
    }
    fn confirm_focus(&mut self, now: Instant) {
        let Some(pending) = self
            .pending_focus
            .as_ref()
            .filter(|pending| pending.due <= now)
        else {
            return;
        };
        let confirmed = self.native_focus_is(pending.window);
        if confirmed || pending.context != self.engine.current || self.space_until.is_some() {
            self.pending_focus = None;
        } else if now >= pending.until {
            self.pending_focus = None;
            self.deadline = Some(now);
        } else {
            self.pending_focus.as_mut().unwrap().due = now + Duration::from_millis(50);
        }
    }
    fn yield_to_system(&mut self, space_changed: bool) {
        if !space_changed {
            self.touch_overview = None;
        }
        self.switch_feedback = false;
        self.space_overview = space_changed;
        if !space_changed || !self.overview_held() {
            self.navigation_display = self.space_transfer_display.or(self.edge_display);
        }
        // Four-finger contact precedes the workspace notification. Abandon the
        // session before its cancellation event can restore an old-Space window.
        self.clear_scroll();
        self.discovery.invalidate_navigation();
        self.pending_focus = None;
        if self.space_transfer_display.is_none() {
            self.pending_commands.clear();
        }
        self.switched_space |= space_changed;
        self.manual_until = None;
        self.space_until = Some(Instant::now() + Duration::from_millis(850));
        self.space_retries = 8;
        self.engine.frozen = true;
        self.overlay_deadline = None;
        if self.space_transfer_display.is_some() {
            if self.overview_held() && self.space_overview {
                self.ui.suspend_overview();
                self.render();
            } else {
                self.ui.reset_overview();
            }
            self.deadline = None;
        } else {
            // Abandon old cards immediately. Only focus/navigation waits for
            // the native gesture; a held overview gets a fresh census now.
            self.overview_space = None;
            self.ui.reset_overview();
            self.deadline = (self.overview_held() && self.space_overview).then(Instant::now);
        }
    }
    fn clear_scroll(&mut self) {
        self.ui.cancel_drag();
        self.click_travel = None;
        self.gesture = None;
        self.pending_swipe = None;
        self.motion_due = None;
        self.linger_until = None;
    }
    fn use_pointer_display(&mut self) {
        self.navigation_display = discovery::pointer_display(self.mtm);
    }
    fn prepare_command(&mut self, command: Command) {
        let navigation = matches!(
            command,
            Command::FocusNext | Command::FocusPrevious | Command::AppNext | Command::AppPrevious
        );
        if navigation {
            if self.navigation_display.is_none() || !self.overview_held() {
                self.use_pointer_display();
            }
            if self
                .navigation_display
                .is_some_and(|display| self.engine.display() != Some(display))
            {
                self.reconcile(false);
                self.deadline = None;
                return;
            }
        } else {
            // Stack edits and Space transfers act on the actual focused window.
            self.navigation_display = None;
        }
        if (navigation || matches!(command, Command::Native(_)))
            && !self.modal
            && self.refresh_navigation_focus()
        {
            return;
        }
        self.reconcile(false);
        self.deadline = None;
    }
    /// Share keyboard's validated Quartz/AX fast path with swipe startup.
    fn refresh_navigation_focus(&mut self) -> bool {
        let Some(observed) = self.discovery.navigation_focus().filter(|id| {
            self.engine
                .context()
                .is_some_and(|context| context.order.contains(id))
        }) else {
            return false;
        };
        let visible: Vec<_> = self
            .discovery
            .handles
            .values()
            .filter(|handle| handle.visible)
            .map(|handle| handle.id)
            .collect();
        let focused = self
            .pending_focus
            .as_ref()
            .map_or(Some(observed), |pending| {
                pending.observed(
                    self.engine.current,
                    &visible,
                    Some(observed),
                    Instant::now(),
                )
            });
        if let Some(focused) = focused {
            self.engine.focus(focused);
        }
        true
    }
    fn select_thumbnail(&mut self, context: u64, window: u64) {
        if !self.ui.is_visible()
            || self.space_until.is_some()
            || self.mouse
            || self.engine.frozen
            || context != self.engine.current
            || !self
                .engine
                .context()
                .is_some_and(|ctx| ctx.order.contains(&window))
        {
            return;
        }
        // Validate against fresh Quartz visibility; indistinguishable overlapping
        // windows also need their application's AX cohort checked.
        if !self.discovery.is_onscreen_on_display(
            window,
            self.engine
                .display()
                .map(|display| (display, &self.engine.bounds)),
        ) {
            self.ui.reset_overview();
            self.deadline = Some(Instant::now());
            return;
        }
        // AX focus may settle after the overview's post-Space census. Refresh
        // the starting window without borrowing another monitor's focus or
        // overriding an accepted step whose AX observation is still pending.
        if self.pending_focus.is_none() {
            if let Some(observed) = self.discovery.navigation_focus().filter(|id| {
                self.engine
                    .context()
                    .is_some_and(|ctx| ctx.order.contains(id))
            }) {
                self.engine.focus(observed);
            }
        }
        let sequence = ClickSequence::new(&self.engine, window);
        self.clear_scroll();
        self.overlay_deadline = None;
        if sequence.as_ref().is_some_and(|sequence| sequence.pending()) {
            let mut sequence = sequence.unwrap();
            let background = self.plan_background_order(&sequence);
            if background.is_some() {
                self.apply(vec![Effect::Focus(window)]);
                if self.engine.focused() != Some(window) {
                    self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
                    self.render();
                    return;
                }
                sequence.accept_destination();
            }
            let immediate = background.is_some();
            self.click_travel = Some(ClickTravel {
                sequence,
                due: Instant::now()
                    + if immediate {
                        Duration::from_millis(8)
                    } else {
                        Duration::ZERO
                    },
                background,
            });
            // Paint the destination before starting any background AX work.
            if !immediate {
                self.advance_click(Instant::now());
            }
        } else {
            self.apply(vec![Effect::Focus(window)]);
            self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        }
        self.deadline = Some(Instant::now() + Duration::from_millis(100));
        self.render();
        self.save();
    }
    fn plan_background_order(&self, sequence: &ClickSequence) -> Option<BackgroundOrder> {
        let windows = sequence.background_windows(|id| {
            self.discovery
                .handles
                .get(&id)
                .filter(|handle| handle.visible && handle.quartz_id.is_some())
                .map(|handle| {
                    (
                        handle.pid,
                        handle.app.bundleIdentifier().map(|id| id.to_string()),
                    )
                })
        })?;
        let display = self.engine.display()?;
        let space = spaces::active_space(display)?;
        if self.overview_space != Some((display, space)) {
            return None;
        }
        let mut ids = vec![sequence.target()];
        ids.extend(&windows);
        if let Some(source) = sequence.selected().filter(|id| !ids.contains(id)) {
            ids.push(source);
        }
        let identities = ids
            .iter()
            .map(|id| {
                let handle = self.discovery.handles.get(id)?;
                Some((*id, handle.pid, handle.quartz_id?))
            })
            .collect::<Option<Vec<_>>>()?;
        self.discovery
            .stacking_order(&ids, display, &self.engine.bounds)?;
        Some(BackgroundOrder {
            windows,
            identities,
            display,
            space,
            epoch: self.ui.visibility_epoch(),
            raised: 0,
            waiting: false,
            confirmed: false,
            until: Instant::now() + Duration::from_millis(600),
        })
    }
    fn advance_background(&mut self, now: Instant) {
        // Take ownership for this timer turn so cancellation never leaves a
        // detached worker issuing raises after new navigation takes over.
        let mut travel = self.click_travel.take().unwrap();
        let repair = travel.background.as_mut().unwrap();
        let target = travel.sequence.target();
        let ids: Vec<_> = repair.identities.iter().map(|(id, _, _)| *id).collect();
        let valid = self.engine.current == travel.sequence.context()
            && self.engine.focused() == Some(target)
            && self.engine.display() == Some(repair.display)
            && self.ui.visibility_epoch() == repair.epoch
            && spaces::active_space(repair.display) == Some(repair.space)
            && repair.identities.iter().all(|(id, pid, quartz)| {
                self.engine
                    .context()
                    .is_some_and(|ctx| ctx.order.contains(id))
                    && self.discovery.handles.get(id).is_some_and(|handle| {
                        handle.pid == *pid && handle.quartz_id == Some(*quartz)
                    })
            });
        let order = valid
            .then(|| {
                self.discovery
                    .stacking_order(&ids, repair.display, &self.engine.bounds)
            })
            .flatten();
        if order.is_none() {
            self.pending_focus = None;
            self.deadline = Some(Instant::now());
            self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
            self.render();
            return;
        }
        let native = self.native_focus_is(target);
        if !native {
            if !repair.confirmed && now < repair.until {
                travel.due = Instant::now() + Duration::from_millis(8);
                self.click_travel = Some(travel);
            } else {
                // Respect external focus; never reclaim it to finish a repair.
                self.pending_focus = None;
                self.deadline = Some(Instant::now());
                self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
            }
            self.render();
            return;
        }
        repair.confirmed = true;
        let prefix_len = repair.raised + usize::from(repair.waiting);
        let expected: Vec<_> = std::iter::once(target)
            .chain(repair.windows[..prefix_len].iter().rev().copied())
            .collect();
        let ordered = order.as_ref().unwrap().starts_with(&expected);
        if !ordered && now < repair.until {
            travel.due = Instant::now() + Duration::from_millis(8);
            self.click_travel = Some(travel);
            return;
        }
        if !ordered {
            // The destination is already visible. Replaying the original path
            // here would flash intervening applications in front of it.
            self.finish_background_order(true);
            return;
        }
        if repair.waiting {
            repair.raised += 1;
            repair.waiting = false;
        }
        if repair.raised == repair.windows.len() {
            self.finish_background_order(false);
            return;
        }
        let id = repair.windows[repair.raised];
        let handle = &self.discovery.handles[&id];
        let code = unsafe {
            AXUIElementSetMessagingTimeout(handle.element.0, 0.05);
            AXUIElementPerformAction(handle.element.0, string("AXRaise").0)
        };
        if code != 0 {
            self.finish_background_order(true);
            return;
        }
        repair.waiting = true;
        repair.until = Instant::now() + Duration::from_millis(250);
        travel.due = Instant::now() + Duration::from_millis(8);
        self.click_travel = Some(travel);
    }
    fn finish_background_order(&mut self, refresh: bool) {
        if refresh {
            // Observe the actual order/focus after a partial repair; do not
            // change native focus merely to finish background bookkeeping.
            self.deadline = Some(Instant::now());
        }
        self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        self.render();
        self.save();
    }
    fn advance_click(&mut self, now: Instant) {
        if !self
            .click_travel
            .as_ref()
            .is_some_and(|travel| travel.due <= now)
        {
            return;
        }
        if self.engine.frozen || self.space_until.is_some() {
            self.clear_scroll();
            self.render();
            return;
        }
        if self
            .click_travel
            .as_ref()
            .is_some_and(|travel| travel.background.is_some())
        {
            self.advance_background(now);
            return;
        }
        let next = self
            .click_travel
            .as_mut()
            .and_then(|travel| travel.sequence.advance(&self.engine));
        if let Some(window) = next {
            // Visit every intervening native window, just as repeated keyboard
            // or swipe steps do, so the compositor retains the same layer order.
            self.apply(vec![Effect::Focus(window)]);
            if self.engine.focused() != Some(window) {
                self.clear_scroll();
                self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
                self.render();
                return;
            }
        }
        if next.is_some()
            && self
                .click_travel
                .as_ref()
                .is_some_and(|travel| travel.sequence.pending())
        {
            // One visible step per timer turn, even after a slow AX call.
            // Retarget the wheel from its live presentation between steps.
            self.click_travel.as_mut().unwrap().due = Instant::now() + Duration::from_millis(70);
            self.render();
            return;
        }
        self.click_travel = None;
        self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        self.render();
        self.save();
    }
    fn touch_overview(&mut self, device: usize, held: bool) {
        if !held {
            if self.touch_overview == Some(device) {
                self.touch_overview = None;
                self.render();
            }
            return;
        }
        if self.mouse
            || self.ui.pointer_active()
            || !self.config.overlay
            || self.touch_overview.is_some_and(|owner| owner != device)
        {
            return;
        }
        if !self.overview_held() {
            self.use_pointer_display();
        }
        self.touch_overview = Some(device);
        // Contact opens a read-only preview. Directional recognition alone
        // creates a navigation session and changes native window focus.
        if self.space_until.is_none() || self.space_overview {
            if self.engine.display() != self.navigation_display
                || self.switched_space
                || self.modal
                || !self.refresh_navigation_focus()
            {
                self.reconcile(false);
                self.deadline = None;
            }
            self.render();
            self.ui.preload_previews(&self.discovery);
        }
    }
    fn contact_began(&mut self) {
        // Resting or repositioning a finger after a click is not navigation.
        // Only a recognized gesture (begin_scroll) interrupts click navigation.
        let catch = self
            .gesture
            .as_ref()
            .is_some_and(|active| active.released || active.source == ScrollSource::Wheel);
        let bridge = catch || self.linger_until.is_some();
        if catch {
            self.gesture = None;
            self.motion_due = None;
            self.save();
        }
        if bridge {
            // Keep the same panel/cards visible through the recognition gap of
            // the next swipe, even if the old feedback deadline is almost over.
            self.linger_until = self
                .linger_until
                .max(Some(Instant::now() + Duration::from_millis(250)));
        }
        self.render();
    }
    fn begin_scroll(&mut self, source: ScrollSource, axis: Axis) {
        let display = if self.overview_held() {
            self.navigation_display
                .or_else(|| discovery::pointer_display(self.mtm))
        } else {
            discovery::pointer_display(self.mtm)
        };
        self.begin_scroll_on_display(source, axis, display);
    }
    fn begin_scroll_on_display(&mut self, source: ScrollSource, axis: Axis, display: Option<u32>) {
        // A new touch/wheel movement immediately catches an existing coast.
        if self.gesture.as_ref().is_some_and(|active| !active.released) {
            return;
        }
        if self.space_until.is_some() || self.mouse {
            return;
        }
        let display_changed = self.navigation_display != display;
        self.navigation_display = display;
        if self.click_travel.take().is_some() {
            self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        }
        let reuse = !display_changed
            && self
                .linger_until
                .is_some_and(|until| until > Instant::now())
            && self.deadline.is_none()
            && !self.switched_space
            && self.discovery.navigation_focus().is_some_and(|id| {
                self.engine
                    .context()
                    .is_some_and(|context| context.order.contains(&id))
            });
        self.gesture = None;
        self.motion_due = None;
        // Do not clear feedback before reconciliation: it can render, and a
        // momentary hidden panel would restart all animations between swipes.
        if !reuse
            && (self.engine.display() != display
                || self.switched_space
                || self.modal
                || !self.refresh_navigation_focus())
        {
            self.reconcile(false);
            self.deadline = None;
        }
        if self.engine.frozen {
            return;
        }
        if let Some(session) = ScrubSession::new(&self.engine, axis) {
            self.gesture = Some(ActiveScroll {
                source,
                session,
                motion: KineticScroll::default(),
                wheel: WheelScroll::default(),
                position: 0.,
                last_time: 0.,
                last_input: Instant::now(),
                released: false,
                changed: false,
                sustained: false,
            });
            self.overlay_deadline = None;
            self.linger_until = None;
            self.render();
        }
    }
    fn select_scroll(&mut self, offset: i32) -> bool {
        let target = self.gesture.as_mut().and_then(|active| {
            active.motion.offset = offset;
            (active.session.context == self.engine.current)
                .then(|| active.session.update(offset))
                .flatten()
        });
        if let Some(id) = target {
            if let Some(active) = self.gesture.as_mut() {
                active.changed = true;
            }
            if !self.engine.frozen
                && self
                    .engine
                    .context()
                    .is_some_and(|ctx| ctx.order.contains(&id))
            {
                self.apply(vec![Effect::Focus(id)]);
            }
        }
        self.render();
        target.is_some()
    }
    fn resume_pending_swipe(&mut self) {
        let Some(pending) = self.pending_swipe.take() else {
            return;
        };
        if spaces::active_space(pending.display) != Some(pending.space)
            || self.engine.display() != Some(pending.display)
            || self.mouse
            || self.modal
        {
            self.render();
            return;
        }
        self.begin_scroll_on_display(
            ScrollSource::Trackpad(pending.device),
            pending.axis,
            Some(pending.display),
        );
        let offset = if pending.released && pending.offset == 0 {
            if pending.position > 0. {
                1
            } else if pending.position < 0. {
                -1
            } else {
                0
            }
        } else {
            pending.offset
        };
        self.select_scroll(offset);
        if pending.released {
            // Do not manufacture release velocity from time spent in a native slide.
            self.release_scroll(pending.time);
        }
    }
    fn gesture_event(&mut self, device: usize, event: GestureEvent, time: f64) {
        if self.ui.pointer_active() {
            return;
        }
        if let Some(pending) = self.pending_swipe.as_mut().filter(|p| p.device == device) {
            match event {
                GestureEvent::Update(offset) => {
                    pending.offset = offset;
                    pending.time = time;
                }
                GestureEvent::End { cancelled: false } => {
                    pending.released = true;
                    pending.time = time;
                }
                GestureEvent::End { cancelled: true } => {
                    self.pending_swipe = None;
                    self.render();
                }
                GestureEvent::Begin(_) => {}
            }
            return;
        }
        match event {
            GestureEvent::Begin(axis) if self.space_until.is_some() => {
                // Only a new swipe after the workspace notification belongs to
                // the destination. Four-finger handoff events remain discarded.
                if !self.space_overview || self.space_transfer_display.is_some() || self.mouse {
                    return;
                }
                let Some(display) = discovery::pointer_display(self.mtm) else {
                    return;
                };
                let Some(space) = spaces::active_space(display) else {
                    return;
                };
                self.navigation_display = Some(display);
                self.reconcile(false);
                self.pending_swipe = Some(PendingSwipe {
                    device,
                    axis,
                    display,
                    space,
                    offset: 0,
                    position: 0.,
                    time,
                    released: false,
                    poll: Instant::now(),
                });
                self.render();
                self.ui.preload_previews(&self.discovery);
            }
            GestureEvent::Begin(axis) => self.begin_scroll(ScrollSource::Trackpad(device), axis),
            GestureEvent::Update(offset) => {
                if self.gesture.as_ref().is_some_and(|active| {
                    active.source == ScrollSource::Trackpad(device) && !active.released
                }) {
                    self.select_scroll(offset);
                }
            }
            GestureEvent::End { cancelled } => {
                if self
                    .gesture
                    .as_ref()
                    .is_none_or(|active| active.source != ScrollSource::Trackpad(device))
                {
                    return;
                }
                if cancelled {
                    let active = self.gesture.take().unwrap();
                    self.clear_scroll();
                    if active.session.context == self.engine.current && self.space_until.is_none() {
                        self.reconcile(false);
                        if active.session.context == self.engine.current {
                            if let Some(id) = active.session.original {
                                self.apply(vec![Effect::Focus(id)]);
                            }
                        }
                    }
                    self.render();
                    self.save();
                } else {
                    // A deliberate short swipe should still make one visible selection.
                    let nudge = self
                        .gesture
                        .as_ref()
                        .and_then(|active| {
                            (!active.changed && active.motion.offset == 0)
                                .then_some(active.motion.direction())
                        })
                        .filter(|direction| *direction != 0);
                    if let Some(offset) = nudge {
                        self.select_scroll(offset);
                    }
                    self.release_scroll(time);
                }
            }
        }
    }
    fn wheel_event(&mut self, delta: f64, time: f64) {
        if self.ui.pointer_active() {
            return;
        }
        if self
            .gesture
            .as_ref()
            .is_none_or(|active| active.source != ScrollSource::Wheel || active.released)
        {
            self.begin_scroll(ScrollSource::Wheel, Axis::Vertical);
        }
        let Some(active) = self
            .gesture
            .as_mut()
            .filter(|active| active.source == ScrollSource::Wheel)
        else {
            return;
        };
        active.position = active.wheel.sample(delta, time);
        active.sustained = active.wheel.sustained();
        active.motion.sample(active.position, time);
        active.last_time = time;
        active.last_input = Instant::now();
        // Wheel packets have no finger-up event. A short quiet interval ends the burst.
        self.motion_due = Some(active.last_input + Duration::from_millis(110));
        let offset = active.position.round() as i32;
        self.select_scroll(offset);
    }
    fn release_scroll(&mut self, time: f64) {
        let coast = self.gesture.as_mut().is_some_and(|active| {
            active.released = true;
            match active.source {
                ScrollSource::Trackpad(_) => active.motion.release(time, active.sustained),
                ScrollSource::Wheel => active.motion.release_wheel(time, active.sustained),
            }
        });
        self.motion_tick = Instant::now();
        if coast {
            self.motion_due = Some(self.motion_tick + Duration::from_millis(16));
        } else {
            self.finish_scroll();
        }
    }
    fn finish_scroll(&mut self) {
        self.gesture = None;
        self.motion_due = None;
        self.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        self.render();
        self.save();
    }
    fn advance_scroll(&mut self, now: Instant) {
        if self.motion_due.is_some_and(|due| due <= now) {
            if let Some(active) = self
                .gesture
                .as_ref()
                .filter(|active| !active.released && active.source == ScrollSource::Wheel)
            {
                let time = active.last_time + now.duration_since(active.last_input).as_secs_f64();
                self.release_scroll(time);
            } else if self.gesture.as_ref().is_some_and(|active| active.released) {
                let dt = now.duration_since(self.motion_tick).as_secs_f64();
                self.motion_tick = now;
                let offset = self
                    .gesture
                    .as_mut()
                    .and_then(|active| active.motion.step(dt));
                if let Some(offset) = offset {
                    if !self.select_scroll(offset) {
                        self.finish_scroll();
                        return;
                    }
                }
                if self
                    .gesture
                    .as_ref()
                    .is_some_and(|active| active.motion.coasting())
                {
                    self.motion_due = Some(now + Duration::from_millis(16));
                } else {
                    self.finish_scroll();
                }
            } else {
                self.motion_due = None;
            }
        }
        if self.linger_until.is_some_and(|until| until <= now) {
            self.linger_until = None;
            self.render();
        }
    }
    fn reload(&mut self) {
        match Config::load(&self.config_path) {
            Ok(c) => {
                if c.gestures != self.config.gestures {
                    if let Some(ScrollSource::Trackpad(device)) =
                        self.gesture.as_ref().map(|active| active.source)
                    {
                        self.gesture_event(device, GestureEvent::End { cancelled: true }, 0.);
                    }
                    self.gestures = None;
                    if c.gestures {
                        self.gestures = trackpad::Monitor::start()
                            .map_err(|e| eprintln!("{e}"))
                            .ok();
                    }
                }
                self.ui.gesture_status(self.gestures.is_some());
                self.config = c;
                self.discovery.invalidate_navigation();
                unsafe {
                    let q = QUEUE.load(Ordering::Acquire);
                    (*q).bindings = self.config.bindings.clone();
                    (*q).scroll_modifiers = self.config.overlay_modifiers;
                    (*q).overview_held = false;
                    (*q).wheel.reset();
                }
                self.clear_scroll();
                self.held = false;
                self.switch_feedback = false;
                self.overlay_deadline = None;
                self.render();
            }
            Err(e) => eprintln!("Configuration unchanged: {e}"),
        }
    }
    fn stage_space_destination(&mut self) {
        let (Some(display), Some(target)) =
            (self.space_transfer_display, self.space_transfer_target)
        else {
            return;
        };
        // Use known metadata while Dock is drawing. A full AX census here can
        // take longer than the animation and delay the entire overview update.
        let visible: Vec<_> = self
            .discovery
            .handles
            .values()
            .filter(|h| {
                if h.minimized || !self.engine.windows.contains_key(&h.id) {
                    return false;
                }
                let membership = h.quartz_id.and_then(|id| spaces::window_space(id).ok());
                membership == Some(Some(target))
                    || (membership == Some(None)
                        && h.visible
                        && self.engine.windows[&h.id].display == display)
            })
            .map(|h| h.id)
            .collect();
        if visible.is_empty() {
            return;
        }
        let context = self.context_tracker.resolve(display, &visible, true);
        if context != self.engine.current {
            self.ui.refresh_overview();
        }
        self.engine.set_display_visible(
            display,
            context,
            &visible,
            self.space_transfer_window.filter(|id| visible.contains(id)),
        );
        self.overview_space = Some((display, target));
        self.render();
    }
    fn reconcile(&mut self, _allow_layout: bool) {
        // Discovery makes synchronous AX calls on the event tap's run loop.
        // A target app can be busy tracking a native drag: defer the census
        // until mouse-up so these calls cannot stall delivery of drag events.
        if self.mouse {
            return;
        }
        let snap = self.discovery.scan(self.mtm, &self.config.ignored_apps);
        self.modal = snap.modal;
        self.engine.frozen = self.mouse || self.modal || self.space_until.is_some();
        self.navigation_display = self
            .navigation_display
            .filter(|display| snap.bounds.contains_key(display));
        let display = snap.active_display(self.navigation_display);
        let native_space = spaces::active_space(display);
        let native_changed =
            native_space.is_some_and(|space| self.overview_space != Some((display, space)));
        let visible: Vec<_> = if let Some(target) = self.space_transfer_target {
            // The compositor can show both Spaces mid-slide. Stage only the
            // destination membership while input stays frozen until completion.
            self.discovery
                .handles
                .values()
                .filter(|h| {
                    let membership = h.quartz_id.and_then(|id| spaces::window_space(id).ok());
                    let sticky_here = membership == Some(None)
                        && h.visible
                        && snap
                            .windows
                            .iter()
                            .chain(self.engine.windows.values())
                            .any(|w| w.id == h.id && w.display == display);
                    (membership == Some(Some(target)) || sticky_here)
                        && !bool_attr(h.element.0, "AXMinimized")
                })
                .map(|h| h.id)
                .collect()
        } else {
            snap.visible_on_display(display)
                .into_iter()
                .filter(|id| {
                    let Some(space) = native_space else {
                        return true;
                    };
                    self.discovery
                        .handles
                        .get(id)
                        .and_then(|h| h.quartz_id)
                        .is_none_or(|quartz| match spaces::window_space(quartz) {
                            Ok(None) => true,
                            Ok(Some(member)) => member == space,
                            Err(_) => false,
                        })
                })
                .collect()
        };
        self.engine.bounds = snap.bounds;
        // A missing AX entry on another Space is not a destroyed window.
        self.engine
            .windows
            .retain(|id, _| self.discovery.handles.contains_key(id));
        let live = self
            .engine
            .windows
            .values()
            .chain(snap.windows.iter())
            .map(|window| (window.id, window.display))
            .collect();
        self.context_tracker.retain(&live);
        let context =
            self.context_tracker
                .resolve(display, &visible, self.switched_space || native_changed);
        let context_changed = self.engine.current != context || self.switched_space;
        let direct_manipulation = (self.mouse && self.dragging)
            || self
                .manual_until
                .is_some_and(|until| until > Instant::now());
        for w in snap.windows {
            let id = w.id;
            let frame = w.frame;
            if let Some(old) = self.engine.windows.get_mut(&id) {
                old.title = w.title;
                old.display = w.display;
                old.app_name = w.app_name;
            } else {
                self.engine.windows.insert(id, w);
            }
            // Reconcile manual changes only in the active context, after Space detection.
            let manual = direct_manipulation
                && !context_changed
                && self.space_until.is_none()
                && visible.contains(&id);
            self.engine.observed_frame(id, frame, manual);
        }
        let mut uninitialized: BTreeMap<_, _> = self
            .engine
            .windows
            .iter()
            .filter(|(id, _)| !self.restored.contains(id))
            .map(|(id, w)| (*id, w.clone()))
            .collect();
        self.recovery.restore_modes(&mut uninitialized);
        for (id, w) in uninitialized {
            if let Some(live) = self.engine.windows.get_mut(&id) {
                live.mode = w.mode;
                if live.mode == whalu::core::Mode::Floating {
                    live.last_floating_frame = Some(live.frame);
                }
            }
            self.restored.insert(id);
        }
        let focused = if self.space_transfer_target.is_some() && self.space_until.is_some() {
            self.space_transfer_window.filter(|id| visible.contains(id))
        } else if snap.modal {
            snap.focused
        } else {
            self.pending_focus.as_ref().map_or(snap.focused, |pending| {
                pending.observed(context, &visible, snap.focused, Instant::now())
            })
        };
        if snap.modal
            || context_changed
            || self.pending_focus.as_ref().is_some_and(|pending| {
                snap.focused == Some(pending.window) || !visible.contains(&pending.window)
            })
        {
            self.pending_focus = None;
        }
        self.engine
            .set_display_visible(display, context, &visible, focused);
        if (context_changed && self.click_travel.is_some())
            || self
                .gesture
                .as_ref()
                .is_some_and(|active| active.session.context != context)
        {
            self.clear_scroll();
        }
        if context_changed {
            self.linger_until = None;
            if self.overview_held() && self.space_overview {
                self.ui.refresh_overview();
            } else {
                self.ui.reset_overview();
            }
        }
        self.switched_space = false;
        self.overview_space = self
            .space_transfer_target
            .or(native_space)
            .map(|space| (display, space));
        self.observe();
        self.render();
        self.save();
        if self.space_retries > 0 {
            if visible.is_empty() {
                self.space_retries -= 1;
                self.deadline = Some(Instant::now() + Duration::from_millis(120));
            } else {
                self.space_retries = 0;
            }
        }
    }
    fn apply(&mut self, effects: Vec<Effect>) {
        if self.mouse || self.space_until.is_some() || self.switched_space {
            return;
        }
        for effect in effects {
            let Effect::Focus(id) = effect;
            let Some(h) = self.discovery.handles.get(&id) else {
                continue;
            };
            // Cached visibility may still describe the previous Space before
            // NSWorkspace delivers its change notification. Never raise or move
            // a window that has already left the screen.
            if !h.visible
                || !self.discovery.is_onscreen_on_display(
                    id,
                    self.engine
                        .display()
                        .map(|display| (display, &self.engine.bounds)),
                )
            {
                self.pending_focus = None;
                self.deadline = Some(Instant::now());
                continue;
            }
            match effect {
                Effect::Focus(id) => unsafe {
                    AXUIElementSetMessagingTimeout(h.element.0, 0.05);
                    if let Some(app) = Owned::new(AXUIElementCreateApplication(h.pid)) {
                        AXUIElementSetMessagingTimeout(app.0, 0.05);
                        AXUIElementSetAttributeValue(
                            app.0,
                            string("AXFocusedWindow").0,
                            h.element.0,
                        );
                    }
                    AXUIElementSetAttributeValue(
                        h.element.0,
                        string("AXFocused").0,
                        kCFBooleanTrue,
                    );
                    let main = AXUIElementSetAttributeValue(
                        h.element.0,
                        string("AXMain").0,
                        kCFBooleanTrue,
                    );
                    let raised = AXUIElementPerformAction(h.element.0, string("AXRaise").0);
                    // Select the exact window before activating its application,
                    // so activation does not restore another window/Space first.
                    #[allow(deprecated)]
                    let activated = h.app.isActive()
                        || h.app.activateWithOptions(
                            NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                        );
                    if activated && (main == 0 || raised == 0) {
                        self.engine.focus(id);
                        let now = Instant::now();
                        self.pending_focus = Some(PendingFocus {
                            context: self.engine.current,
                            window: id,
                            due: now + Duration::from_millis(40),
                            until: now + Duration::from_millis(600),
                        });
                    } else {
                        eprintln!("Could not focus window {id}: AX {main}/{raised}");
                    }
                },
            }
        }
    }
    fn observe(&mut self) {
        unsafe {
            let apps = NSWorkspace::sharedWorkspace().runningApplications();
            let live: BTreeSet<_> = apps.iter().map(|a| a.processIdentifier()).collect();
            self.observers.retain(|pid, _| live.contains(pid));
            for app in apps {
                let pid = app.processIdentifier();
                if pid == std::process::id() as i32
                    || app.activationPolicy() == NSApplicationActivationPolicy::Prohibited
                {
                    continue;
                }
                if app
                    .bundleIdentifier()
                    .is_some_and(|s| self.config.ignored_apps.contains(&s.to_string()))
                {
                    continue;
                }
                if let std::collections::btree_map::Entry::Vacant(entry) = self.observers.entry(pid)
                {
                    let Some(ax) = Owned::new(AXUIElementCreateApplication(pid)) else {
                        continue;
                    };
                    AXUIElementSetMessagingTimeout(ax.0, 0.08);
                    let mut raw = null();
                    if AXObserverCreate(pid, ax_callback, &mut raw) != 0 {
                        continue;
                    }
                    let value = Owned(raw);
                    let source = AXObserverGetRunLoopSource(value.0);
                    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
                    for name in [
                        "AXWindowCreated",
                        "AXFocusedWindowChanged",
                        "AXMainWindowChanged",
                        "AXApplicationActivated",
                        "AXApplicationHidden",
                        "AXApplicationShown",
                    ] {
                        AXObserverAddNotification(value.0, ax.0, string(name).0, null_mut());
                    }
                    entry.insert(Observer {
                        value,
                        source,
                        registered: BTreeSet::new(),
                    });
                }
                let observer = self.observers.get_mut(&pid).unwrap();
                observer
                    .registered
                    .retain(|id| self.discovery.handles.contains_key(id));
                for h in self.discovery.handles.values().filter(|h| h.pid == pid) {
                    if !observer.registered.insert(h.id) {
                        continue;
                    }
                    for name in [
                        "AXUIElementDestroyed",
                        "AXMoved",
                        "AXResized",
                        "AXTitleChanged",
                        "AXWindowMiniaturized",
                        "AXWindowDeminiaturized",
                    ] {
                        AXObserverAddNotification(
                            observer.value.0,
                            h.element.0,
                            string(name).0,
                            null_mut(),
                        );
                    }
                }
            }
        }
    }
    fn render(&self) {
        // Native clicks/drags own the pointer until release. Thumbnail drags
        // are consumed by the tap and never enter this mouse-down state.
        if self.mouse {
            self.ui.hide();
            return;
        }
        self.ui.validate_drag(&self.engine);
        let held = (self.held && self.overlay_deadline.is_none())
            || self.edge_display.is_some()
            || self.touch_overview.is_some();
        if self.config.overlay
            && (held
                || self.gesture.is_some()
                || self.pending_swipe.is_some()
                || self.click_travel.is_some()
                || self.linger_until.is_some()
                || self.ui.pointer_active()
                || self.ui.is_visible())
            && self.space_transfer_target.is_none()
        {
            if let Some((display, space)) = self
                .engine
                .display()
                .and_then(|display| spaces::active_space(display).map(|space| (display, space)))
            {
                if self.overview_space != Some((display, space)) {
                    self.ui.reset_overview();
                    if !self.overview_refresh_pending.replace(true) {
                        enqueue(Event::OverviewRefresh);
                    }
                    return;
                }
            }
        }
        if self.space_until.is_some()
            && self.space_overview
            && (held || self.pending_swipe.is_some())
            && self.config.overlay
        {
            // Fresh overview data can render while focus remains paused;
            // discard click targets until the native transition settles.
            self.ui.sync_sources(&self.discovery);
            self.ui.show(&self.engine);
            self.ui.suspend_overview();
            return;
        }
        if (held
            || self.gesture.is_some()
            || self.pending_swipe.is_some()
            || self.click_travel.is_some()
            || self.linger_until.is_some()
            || self.ui.pointer_active())
            && self.space_until.is_none()
            && self.config.overlay
        {
            self.ui.sync_sources(&self.discovery);
            self.ui.show_selection(
                &self.engine,
                self.gesture.as_ref().map(|active| active.session.axis),
                self.click_travel
                    .as_ref()
                    .and_then(|travel| travel.sequence.selected()),
            );
        } else {
            self.ui.dismiss();
        }
    }
    fn save(&mut self) {
        if let Some(writer) = self.state_writer.as_ref() {
            writer.save(SavedState::capture(&self.engine));
        }
    }
}
unsafe extern "C" fn ax_callback(_: Ref, _: Ref, notification: Ref, _: MutRef) {
    let focus_only = matches!(
        text(notification).as_deref(),
        Some("AXFocusedWindowChanged" | "AXMainWindowChanged" | "AXApplicationActivated")
    );
    enqueue(if focus_only {
        Event::FocusChanged
    } else {
        Event::Dirty
    });
}
unsafe extern "C" fn timer_callback(_: Ref, _: MutRef) {
    wake();
}
unsafe extern "C" fn perform(info: MutRef) {
    // This source is installed only on AppKit's main run loop. No other callback accesses Runtime.
    let q = &*(info as *const Queue);
    objc2::rc::autoreleasepool(|_| {
        (&mut *q.runtime).drain();
    });
}
fn wake() {
    let ptr = QUEUE.load(Ordering::Acquire);
    if !ptr.is_null() {
        unsafe {
            CFRunLoopSourceSignal((*ptr).source);
            CFRunLoopWakeUp(CFRunLoopGetMain());
        }
    }
}
unsafe extern "C" fn event_callback(_: MutRef, ty: u32, event: MutRef, _: MutRef) -> MutRef {
    if ty == 0xffff_fffe || ty == 0xffff_ffff {
        let queue = QUEUE.load(Ordering::Acquire);
        if !queue.is_null() {
            (*queue).shortcuts = ShortcutRouter::default();
        }
        enqueue(Event::TapDisabled);
        return event;
    }
    if CGEventGetIntegerValueField(event, 42) == spaces::DOCK_INPUT_TAG {
        return event;
    }
    let modifiers = Modifiers::from_flags(CGEventGetFlags(event));
    if matches!(ty, 5 | 6 | 7 | 27) {
        enqueue(Event::PointerMoved {
            point: CGEventGetLocation(event),
            dragging: ty != 5,
        });
    }
    if matches!(ty, 1 | 2 | 6) {
        let q = QUEUE.load(Ordering::Acquire);
        if !q.is_null() {
            let point = CGEventGetLocation(event);
            match (*q)
                .clicks
                .route(ty, objc2_foundation::NSPoint::new(point.x, point.y))
            {
                ui::ClickRoute::Pass => {}
                ui::ClickRoute::Consume => return null_mut(),
                ui::ClickRoute::Pointer {
                    context,
                    window,
                    epoch,
                    kind,
                    point,
                } => {
                    enqueue(Event::ThumbnailPointer {
                        context,
                        window,
                        epoch,
                        kind,
                        point,
                    });
                    return null_mut();
                }
                ui::ClickRoute::Select {
                    context,
                    window,
                    epoch,
                } => {
                    enqueue(Event::SelectThumbnail {
                        context,
                        window,
                        epoch,
                    });
                    return null_mut();
                }
            }
        }
    }
    if ty == 22 {
        let continuous = CGEventGetIntegerValueField(event, 88) != 0;
        let momentum = CGEventGetIntegerValueField(event, 123);
        if trackpad::suppress_scroll(continuous, momentum) {
            return null_mut();
        }
        let q = QUEUE.load(Ordering::Acquire);
        if !q.is_null() {
            let delta = if continuous {
                CGEventGetDoubleValueField(event, 96)
            } else {
                CGEventGetIntegerValueField(event, 11) as f64
            };
            let now = (*q).clock.elapsed().as_secs_f64();
            let point = CGEventGetLocation(event);
            let held =
                (*q).scroll_held(modifiers, objc2_foundation::NSPoint::new(point.x, point.y));
            match (*q).wheel.route(
                held,
                delta,
                continuous,
                momentum,
                CGEventGetIntegerValueField(event, 99),
                now,
            ) {
                WheelRoute::Pass => {}
                WheelRoute::Consume => return null_mut(),
                WheelRoute::Navigate(delta) => {
                    enqueue(Event::Wheel(delta, now));
                    return null_mut();
                }
            }
        }
    }
    if ty == 12 {
        let q = QUEUE.load(Ordering::Acquire);
        if !q.is_null() {
            (*q).update_overview_hold(modifiers);
        }
    }
    if ty == 6 {
        enqueue(Event::Drag);
    }
    if ty == 1 {
        enqueue(Event::Mouse(true));
    }
    if ty == 2 {
        enqueue(Event::Mouse(false));
    }
    if ty == 10 || ty == 11 {
        let q = QUEUE.load(Ordering::Acquire);
        if !q.is_null() {
            let key = CGEventGetIntegerValueField(event, 9) as u16;
            let route = (*q)
                .shortcuts
                .key(&(*q).bindings, key, CGEventGetFlags(event), ty == 10);
            (*q).update_overview_hold(modifiers);
            match route {
                KeyRoute::Pass => {}
                KeyRoute::Consume(command) => {
                    if let Some(command) = command {
                        if [48, 50].contains(&key)
                            && modifiers.command
                            && matches!(command, Command::FocusNext | Command::FocusPrevious)
                        {
                            enqueue(Event::WindowSwitch(command));
                        } else {
                            enqueue(Event::Command(command));
                        }
                    }
                    return null_mut();
                }
                KeyRoute::Native(shortcut) => {
                    if ty == 10 {
                        enqueue(Event::Command(Command::Native(shortcut)));
                    }
                    return null_mut();
                }
            }
        }
    }
    event
}

pub fn doctor() -> bool {
    unsafe {
        let trusted = AXIsProcessTrusted();
        println!(
            "Accessibility permission: {}",
            if trusted { "granted" } else { "REQUIRED" }
        );
        println!(
            "Native Space IDs: unavailable through public APIs; conservative visible-window contexts"
        );
        println!(
            "Three-finger contact listener: experimental private framework; owned scroll packets suppressed by the public event tap"
        );
        println!("Config: {}", config_dir().join("config.toml").display());
        trusted
    }
}
pub fn list() -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(permission_help());
    }
    let mtm = MainThreadMarker::new().ok_or("macOS adapter must run on main thread")?;
    let _app = NSApplication::sharedApplication(mtm);
    let mut d = Discovery::new();
    let s = d.scan(mtm, &[]);
    for w in s.windows {
        if s.visible.contains(&w.id) {
            println!(
                "{}\t{}\t{}\t{:.0},{:.0} {:.0}×{:.0}",
                w.id, w.app_name, w.title, w.frame.x, w.frame.y, w.frame.width, w.frame.height
            );
        }
    }
    Ok(())
}
fn permission_help() -> String {
    "Accessibility permission is required. Open System Settings → Privacy & Security → Accessibility, add Whalu.app (or the terminal when using cargo run), enable it, then relaunch. Use `whalu --permissions` to open settings.".into()
}
pub fn run(config: Config, config_path: std::path::PathBuf) -> Result<(), String> {
    unsafe {
        let mtm = MainThreadMarker::new().ok_or("Must run on main thread")?;
        std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(config_dir().join("instance.lock"))
            .map_err(|e| e.to_string())?;
        use std::os::fd::AsRawFd;
        if libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 {
            return Err("Whalu is already running".into());
        }
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        let mask = (1 << 22)
            | (1 << 1)
            | (1 << 2)
            | (1 << 5)
            | (1 << 6)
            | (1 << 7)
            | (1 << 27)
            | (1 << 10)
            | (1 << 11)
            | (1 << 12);
        let Some(tap) = onboarding::wait(mtm, mask) else {
            return Ok(());
        };
        let (tx, rx) = mpsc::channel();
        let mut queue = Box::new(Queue {
            tx,
            source: null(),
            bindings: config.bindings.clone(),
            shortcuts: ShortcutRouter::default(),
            runtime: null_mut(),
            scroll_modifiers: config.overlay_modifiers,
            overview_held: false,
            wheel: WheelRouter::default(),
            clock: Instant::now(),
            clicks: ui::ThumbnailClicks::default(),
        });
        let ptr = &mut *queue as *mut Queue;
        let mut context = SourceContext {
            version: 0,
            info: ptr.cast(),
            retain: None,
            release: None,
            description: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: Some(perform),
        };
        let source = Owned::new(CFRunLoopSourceCreate(null(), 0, &mut context))
            .ok_or("Cannot create main run-loop source")?;
        queue.source = source.0;
        CFRunLoopAddSource(CFRunLoopGetMain(), source.0, kCFRunLoopCommonModes);
        let tap_source = Owned::new(CFMachPortCreateRunLoopSource(null(), tap.0, 0))
            .ok_or("Cannot create keyboard run-loop source")?;
        CFRunLoopAddSource(CFRunLoopGetMain(), tap_source.0, kCFRunLoopCommonModes);
        CGEventTapEnable(tap.0, true);
        let ui = ui::Ui::new(mtm);
        queue.clicks = ui.clicks.clone();
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        for name in [
            "NSWorkspaceDidLaunchApplicationNotification",
            "NSWorkspaceDidTerminateApplicationNotification",
            "NSWorkspaceDidActivateApplicationNotification",
            "NSWorkspaceDidHideApplicationNotification",
            "NSWorkspaceDidUnhideApplicationNotification",
            "NSWorkspaceDidWakeNotification",
        ] {
            center.addObserver_selector_name_object(
                &ui.delegate,
                sel!(changed:),
                Some(&NSString::from_str(name)),
                None,
            );
        }
        center.addObserver_selector_name_object(
            &ui.delegate,
            sel!(spaceChanged:),
            Some(&NSString::from_str(
                "NSWorkspaceActiveSpaceDidChangeNotification",
            )),
            None,
        );
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            &ui.delegate,
            sel!(displaysChanged:),
            Some(&NSString::from_str(
                "NSApplicationDidChangeScreenParametersNotification",
            )),
            None,
        );
        let gestures_enabled = config.gestures;
        let mut runtime = Box::new(Runtime {
            mtm,
            rx,
            engine: Engine::new(config.gap),
            config,
            config_path,
            state_writer: Some(
                StateWriter::new(config_dir().join("state.json")).map_err(|e| e.to_string())?,
            ),
            discovery: Discovery::new(),
            ui,
            observers: BTreeMap::new(),
            gesture: None,
            pending_swipe: None,
            click_travel: None,
            pending_focus: None,
            motion_due: None,
            linger_until: None,
            motion_tick: Instant::now(),
            gestures: if gestures_enabled {
                match trackpad::Monitor::start() {
                    Ok(m) => {
                        eprintln!("Three-finger gestures listening on {} trackpad(s)", m.count);
                        Some(m)
                    }
                    Err(e) => {
                        eprintln!("Three-finger gestures unavailable: {e}");
                        None
                    }
                }
            } else {
                None
            },
            tap,
            _tap_source: tap_source,
            timer: None,
            deadline: None,
            overlay_deadline: None,
            held: false,
            edge_display: None,
            touch_overview: None,
            switch_feedback: false,
            mouse: false,
            dragging: false,
            manual_until: None,
            switched_space: false,
            pending_commands: vec![],
            space_until: None,
            space_retries: 0,
            space_transfer_display: None,
            space_transfer_target: None,
            space_transfer_window: None,
            space_overview: false,
            overview_space: None,
            overview_refresh_pending: Cell::new(false),
            modal: false,
            recovery: SavedState::load(&config_dir().join("state.json")),
            restored: BTreeSet::new(),
            context_tracker: DisplayContexts::default(),
            navigation_display: None,
            _lock: lock,
        });
        queue.runtime = &mut *runtime;
        *DISPATCH.lock().unwrap() = Some((queue.tx.clone(), source.0 as usize));
        QUEUE.store(ptr, Ordering::Release);
        runtime.ui.gesture_status(runtime.gestures.is_some());
        runtime.reconcile(false);
        eprintln!(
            "Whalu running. Hold Control + Option to navigate; use the W menu to refresh or quit."
        );
        app.run();
        *DISPATCH.lock().unwrap() = None;
        QUEUE.store(null_mut(), Ordering::Release);
        drop(runtime);
        drop(queue);
        drop(source);
        Ok(())
    }
}

pub fn gesture_probe(seconds: f64) -> Result<(), String> {
    trackpad::probe(seconds)
}

#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn accepted_focus_is_not_erased_by_a_late_or_empty_ax_observation() {
        let now = Instant::now();
        let pending = PendingFocus {
            context: 1,
            window: 4,
            due: now,
            until: now + Duration::from_millis(600),
        };
        assert_eq!(pending.observed(1, &[1, 2, 3, 4], Some(1), now), Some(4));
        assert_eq!(pending.observed(1, &[1, 2, 3, 4], None, now), Some(4));
        assert_eq!(pending.observed(1, &[1, 2, 3, 4], Some(4), now), Some(4));
        // No intent may carry its window across Spaces, disappearance, or expiry.
        assert_eq!(pending.observed(2, &[4, 5], Some(5), now), Some(5));
        assert_eq!(pending.observed(1, &[1, 2, 3], Some(2), now), Some(2));
        assert_eq!(
            pending.observed(1, &[1, 2, 3, 4], Some(1), now + Duration::from_millis(600)),
            Some(1)
        );
    }
}
