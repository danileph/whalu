//! All native callbacks enqueue work; Accessibility calls never run in the event tap.
#![allow(unsafe_op_in_unsafe_fn)]
mod discovery;
mod ffi;
mod onboarding;
mod previews;
pub mod probe;
mod trackpad;
mod ui;
use discovery::Discovery;
use ffi::*;
use magnetic::{
    config::{Config, config_dir},
    core::{Command, Effect, Engine},
    gestures::GestureEvent,
    input::{Modifiers, route},
    navigation::ScrubSession,
    persistence::SavedState,
    visibility::ContextTracker,
};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy, NSWorkspace,
};
use objc2_foundation::{NSNotificationCenter, NSString};
use std::{
    collections::{BTreeMap, BTreeSet},
    ptr::{null, null_mut},
    sync::{
        Mutex,
        atomic::{AtomicPtr, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

static QUEUE: AtomicPtr<Queue> = AtomicPtr::new(null_mut());
// Protect dispatch lifetime for trackpad callbacks arriving on a framework thread.
static DISPATCH: Mutex<Option<(Sender<Event>, usize)>> = Mutex::new(None);
#[derive(Debug)]
pub(super) enum Event {
    Dirty,
    SpaceChanged,
    Command(Command),
    Modifiers(Modifiers),
    Mouse(bool),
    Drag,
    Gesture(usize, GestureEvent),
    TogglePause,
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
    bindings: Vec<magnetic::config::Binding>,
    runtime: *mut Runtime,
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
struct Runtime {
    mtm: MainThreadMarker,
    rx: Receiver<Event>,
    config: Config,
    config_path: std::path::PathBuf,
    engine: Engine,
    discovery: Discovery,
    ui: ui::Ui,
    observers: BTreeMap<i32, Observer>,
    tap: Owned,
    gestures: Option<trackpad::Monitor>,
    gesture: Option<(usize, ScrubSession)>,
    _tap_source: Owned,
    timer: Option<Owned>,
    deadline: Option<Instant>,
    overlay_deadline: Option<Instant>,
    held: bool,
    mouse: bool,
    dragging: bool,
    manual_until: Option<Instant>,
    switched_space: bool,
    pending_commands: Vec<Command>,
    space_until: Option<Instant>,
    paused: bool,
    modal: bool,
    recovery: SavedState,
    restored: BTreeSet<u64>,
    context_tracker: ContextTracker,
    last_saved: Option<String>,
    _lock: std::fs::File,
}
impl Runtime {
    fn schedule(&mut self) {
        unsafe {
            if let Some(t) = self.timer.take() {
                CFRunLoopTimerInvalidate(t.0);
            }
            let scan_due = if self.space_until.is_none() {
                self.deadline
            } else {
                None
            };
            let due = [scan_due, self.overlay_deadline, self.space_until]
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
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Dirty => self.deadline = Some(Instant::now() + Duration::from_millis(70)),
                Event::SpaceChanged => {
                    // Native Space navigation wins over an in-progress scrub.
                    self.gesture = None;
                    self.switched_space = true;
                    self.manual_until = None;
                    self.space_until = Some(Instant::now() + Duration::from_millis(850));
                    self.engine.frozen = true;
                    self.ui.hide();
                    self.deadline = None;
                }
                Event::Modifiers(m) => {
                    let held = m.overlay(self.config.overlay_modifiers);
                    if held && !self.held && self.config.overlay {
                        self.overlay_deadline = Some(
                            Instant::now() + Duration::from_millis(self.config.overlay_delay_ms),
                        );
                    }
                    if !held {
                        self.overlay_deadline = None;
                    }
                    self.held = held;
                    self.render();
                }
                Event::Gesture(device, event) => self.gesture_event(device, event),
                Event::Drag => self.dragging = true,
                Event::Mouse(down) => {
                    if down {
                        self.dragging = false;
                    }
                    if !down && self.dragging {
                        self.manual_until = Some(Instant::now() + Duration::from_millis(350));
                        self.dragging = false;
                    }
                    self.mouse = down;
                    self.engine.frozen = down || self.modal || self.space_until.is_some();
                    if !down {
                        self.deadline = Some(Instant::now() + Duration::from_millis(90));
                    }
                }
                Event::Command(cmd) => {
                    if self.space_until.is_some() {
                        if self.pending_commands.len() < 32 {
                            self.pending_commands.push(cmd);
                        }
                        continue;
                    }
                    if self.mouse {
                        continue;
                    }
                    // Navigation is explicit intent: bypass the modifier-only HUD delay.
                    self.overlay_deadline = None;
                    self.reconcile(false);
                    self.deadline = None;
                    if std::env::var_os("MAGNETIC_TRACE").is_some() {
                        eprintln!(
                            "Command {cmd:?}: context={}, windows={}, focused={:?}",
                            self.engine.current,
                            self.engine.context().map_or(0, |c| c.order.len()),
                            self.engine.focused()
                        );
                    }
                    let old = self.engine.focused();
                    let effects = self.engine.command(cmd);
                    self.apply(effects);
                    if !matches!(cmd, Command::Snap(_)) && self.engine.focused() != old {
                        self.reflow();
                    }
                    if matches!(cmd, Command::ToggleMode)
                        && self
                            .engine
                            .focused()
                            .and_then(|id| self.engine.windows.get(&id))
                            .is_some_and(|w| w.mode == magnetic::core::Mode::Magnetic)
                    {
                        self.reflow();
                    }
                    self.render();
                    self.save();
                }
                Event::TogglePause => {
                    self.paused = !self.paused;
                    self.ui.paused(self.paused);
                }
                Event::PreviewAccess => previews::request_access(),
                Event::PreviewReady(id, quartz, image) => {
                    self.ui.preview_ready(id, quartz, image);
                    self.render();
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
                    self.ui.hide();
                    NSApplication::sharedApplication(self.mtm).terminate(None);
                }
                Event::TapDisabled => {
                    self.held = false;
                    self.overlay_deadline = None;
                    self.render();
                    unsafe {
                        CGEventTapEnable(self.tap.0, true);
                    }
                }
            }
        }
        let now = Instant::now();
        if self.space_until.is_some_and(|d| d <= now) {
            self.space_until = None;
            self.deadline = None;
            self.reconcile(false);
            for cmd in std::mem::take(&mut self.pending_commands) {
                enqueue(Event::Command(cmd));
            }
        } else if self.space_until.is_none() && self.deadline.is_some_and(|d| d <= now) {
            self.deadline = None;
            self.reconcile(true);
        }
        if self.overlay_deadline.is_some_and(|d| d <= now) {
            self.overlay_deadline = None;
            self.render();
        }
        self.schedule();
    }
    fn gesture_event(&mut self, device: usize, event: GestureEvent) {
        match event {
            GestureEvent::Begin(axis) => {
                if self.gesture.is_some() || self.space_until.is_some() || self.mouse {
                    return;
                }
                self.reconcile(false);
                if self.engine.frozen {
                    return;
                }
                if let Some(session) = ScrubSession::new(&self.engine, axis) {
                    self.gesture = Some((device, session));
                    self.overlay_deadline = None;
                    self.render();
                    if std::env::var_os("MAGNETIC_TRACE").is_some() {
                        eprintln!("Continuous gesture started: {axis:?}");
                    }
                }
            }
            GestureEvent::Update(offset) => {
                let target = match self.gesture.as_mut() {
                    Some((active, session))
                        if *active == device && session.context == self.engine.current =>
                    {
                        session.update(offset)
                    }
                    _ => return,
                };
                if let Some(id) = target {
                    if self.engine.context().is_some_and(|c| c.order.contains(&id))
                        && !self.engine.frozen
                    {
                        self.apply(vec![Effect::Focus(id)]);
                        if std::env::var_os("MAGNETIC_TRACE").is_some() {
                            eprintln!("Continuous gesture selection: offset={offset}, window={id}");
                        }
                    }
                }
                self.render();
            }
            GestureEvent::End { cancelled } => {
                if self
                    .gesture
                    .as_ref()
                    .is_none_or(|(active, _)| *active != device)
                {
                    return;
                }
                let (_, session) = self.gesture.take().unwrap();
                if cancelled {
                    if session.context == self.engine.current && self.space_until.is_none() {
                        // Refresh visibility before restoring; never jump back across Spaces.
                        self.reconcile(false);
                        if session.context == self.engine.current {
                            if let Some(id) = session.original {
                                self.apply(vec![Effect::Focus(id)]);
                            }
                        }
                    }
                } else {
                    self.reflow();
                }
                self.render();
                self.save();
                if std::env::var_os("MAGNETIC_TRACE").is_some() {
                    eprintln!("Continuous gesture ended: cancelled={cancelled}");
                }
            }
        }
    }
    fn reload(&mut self) {
        match Config::load(&self.config_path) {
            Ok(c) => {
                self.engine.gap = c.gap;
                if c.gestures != self.config.gestures {
                    if let Some((device, _)) = self.gesture.as_ref() {
                        self.gesture_event(*device, GestureEvent::End { cancelled: true });
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
                unsafe {
                    let q = QUEUE.load(Ordering::Acquire);
                    (*q).bindings = self.config.bindings.clone();
                }
                self.held = false;
                self.overlay_deadline = None;
                self.render();
            }
            Err(e) => eprintln!("Configuration unchanged: {e}"),
        }
    }
    fn reconcile(&mut self, allow_layout: bool) {
        let snap = self.discovery.scan(self.mtm, &self.config.ignored_apps);
        let previous = self.engine.focused();
        self.modal = snap.modal;
        self.engine.frozen = self.mouse || self.modal || self.space_until.is_some();
        self.engine.bounds = snap.bounds;
        let context = self
            .context_tracker
            .resolve(&snap.visible, self.switched_space);
        let context_changed = self.engine.current != context || self.switched_space;
        // A missing AX entry on another Space is not a destroyed window.
        self.engine
            .windows
            .retain(|id, _| self.discovery.handles.contains_key(id));
        let live: BTreeSet<_> = self
            .engine
            .windows
            .keys()
            .copied()
            .chain(snap.windows.iter().map(|w| w.id))
            .collect();
        self.context_tracker.retain(&live);
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
                && snap.visible.contains(&id);
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
                if live.mode == magnetic::core::Mode::Floating {
                    live.last_floating_frame = Some(live.frame);
                }
            }
            self.restored.insert(id);
        }
        self.engine
            .set_visible(context, &snap.visible, snap.focused);
        if self
            .gesture
            .as_ref()
            .is_some_and(|(_, s)| s.context != context)
        {
            self.gesture = None;
        }
        self.switched_space = false;
        self.observe();
        let changed = previous != snap.focused;
        if allow_layout && changed && !context_changed && !direct_manipulation {
            self.reflow();
        }
        self.render();
        self.save();
    }
    fn reflow(&mut self) {
        if !self.paused && self.config.magnetic && self.gesture.is_none() {
            self.apply(self.engine.plan());
        }
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            let id = match effect {
                Effect::Focus(id) | Effect::Move(id, _) | Effect::Snap(id, _) => id,
            };
            let Some(h) = self.discovery.handles.get(&id) else {
                continue;
            };
            if !h.visible {
                continue;
            }
            match effect {
                Effect::Focus(id) => unsafe {
                    #[allow(deprecated)]
                    let activated = h.app.activateWithOptions(
                        NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                    );
                    if let Some(app) = Owned::new(AXUIElementCreateApplication(h.pid)) {
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
                    if activated && (main == 0 || raised == 0) {
                        self.engine.focus(id);
                    } else {
                        eprintln!("Could not focus window {id}: AX {main}/{raised}");
                    }
                },
                Effect::Move(id, r) => {
                    let Some(w) = self.engine.windows.get(&id) else {
                        continue;
                    };
                    // Enforce M-01 at the native boundary, independently of the solver.
                    if w.mode != magnetic::core::Mode::Magnetic
                        || (w.frame.width - r.width).abs() > 0.01
                        || (w.frame.height - r.height).abs() > 0.01
                    {
                        continue;
                    }
                    if let Err(code) = set_position(h.element.0, r) {
                        eprintln!("Window {id}: move rejected (AX {code})");
                    }
                    if let Some(actual) = frame(h.element.0) {
                        self.engine.observed_frame(id, actual, false);
                    }
                }
                Effect::Snap(id, r) => {
                    if !settable(h.element.0, "AXSize") {
                        eprintln!("Window {id} does not support resizing");
                        self.engine.clear_snap(id);
                        continue;
                    }
                    // Respect the app-accepted size, then anchor it inside this display.
                    let bounds = self
                        .engine
                        .windows
                        .get(&id)
                        .and_then(|w| self.engine.bounds.get(&w.display))
                        .copied()
                        .unwrap_or(r);
                    let result = snap_frame(h.element.0, r, bounds);
                    if let Err(code) = result {
                        eprintln!("Window {id}: snap constrained/rejected (AX {code})");
                    }
                    if let Some(actual) = frame(h.element.0) {
                        if !actual.near(r) {
                            self.engine.clear_snap(id);
                        }
                        self.engine.observed_frame(id, actual, false);
                    }
                }
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
        let keyboard = self.held && self.overlay_deadline.is_none();
        if (keyboard || self.gesture.is_some()) && self.space_until.is_none() && self.config.overlay
        {
            self.ui.sync_sources(&self.discovery);
            self.ui
                .show_navigation(&self.engine, self.gesture.as_ref().map(|(_, s)| s.axis));
        } else {
            self.ui.hide();
        }
    }
    fn save(&mut self) {
        let state = SavedState::capture(&self.engine);
        if let Ok(data) = serde_json::to_string(&state) {
            if self.last_saved.as_ref() == Some(&data) {
                return;
            }
            match state.write(&config_dir().join("state.json")) {
                Ok(()) => self.last_saved = Some(data),
                Err(e) => eprintln!("Recovery write failed: {e}"),
            }
        }
    }
}
unsafe extern "C" fn ax_callback(_: Ref, _: Ref, _: Ref, _: MutRef) {
    enqueue(Event::Dirty);
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
        enqueue(Event::TapDisabled);
        return event;
    }
    let modifiers = Modifiers::from_flags(CGEventGetFlags(event));
    if ty == 12 {
        enqueue(Event::Modifiers(modifiers));
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
            if let Some(cmd) = route(&(*q).bindings, key, modifiers) {
                if ty == 10 {
                    enqueue(Event::Command(cmd));
                }
                return null_mut();
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
            "Global three-finger gestures: read-only trackpad listener (experimental private framework)"
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
    "Accessibility permission is required. Open System Settings → Privacy & Security → Accessibility, add Magnetic.app (or the terminal when using cargo run), enable it, then relaunch. Use `magnetic --permissions` to open settings.".into()
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
            return Err("Magnetic is already running".into());
        }
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        let mask = (1 << 1) | (1 << 2) | (1 << 6) | (1 << 10) | (1 << 11) | (1 << 12);
        let Some(tap) = onboarding::wait(mtm, mask) else {
            return Ok(());
        };
        let (tx, rx) = mpsc::channel();
        let mut queue = Box::new(Queue {
            tx,
            source: null(),
            bindings: config.bindings.clone(),
            runtime: null_mut(),
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
            sel!(spaceChanged:),
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
            discovery: Discovery::new(),
            ui,
            observers: BTreeMap::new(),
            gesture: None,
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
            mouse: false,
            dragging: false,
            manual_until: None,
            switched_space: false,
            pending_commands: vec![],
            space_until: None,
            paused: false,
            modal: false,
            recovery: SavedState::load(&config_dir().join("state.json")),
            restored: BTreeSet::new(),
            context_tracker: ContextTracker::default(),
            last_saved: None,
            _lock: lock,
        });
        queue.runtime = &mut *runtime;
        *DISPATCH.lock().unwrap() = Some((queue.tx.clone(), source.0 as usize));
        QUEUE.store(ptr, Ordering::Release);
        runtime.ui.gesture_status(runtime.gestures.is_some());
        runtime.reconcile(false);
        eprintln!(
            "Magnetic running. Hold Control + Option to navigate; use the M menu to pause or quit."
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
