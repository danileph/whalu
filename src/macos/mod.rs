//! All native callbacks enqueue work; Accessibility calls never run in the event tap.
#![allow(unsafe_op_in_unsafe_fn)]
mod discovery;
mod ffi;
mod input_probe;
mod onboarding;
mod previews;
pub mod probe;
mod spaces;
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
    input::{Modifiers, route},
    navigation::{ClickSequence, ScrubSession},
    persistence::{SavedState, StateWriter},
    scroll::{KineticScroll, WheelRoute, WheelRouter, WheelScroll},
    visibility::ContextTracker,
};

static QUEUE: AtomicPtr<Queue> = AtomicPtr::new(null_mut());
// Protect dispatch lifetime for trackpad callbacks arriving on a framework thread.
static DISPATCH: Mutex<Option<(Sender<Event>, usize)>> = Mutex::new(None);
#[derive(Debug)]
pub(super) enum Event {
    Dirty,
    SpaceChanged,
    NativeGesture,
    Command(Command),
    Modifiers(Modifiers),
    Mouse(bool),
    SelectThumbnail {
        context: u64,
        window: u64,
        epoch: u64,
    },
    Drag,
    Gesture(usize, GestureEvent, f64),
    GestureMotion(usize, f64, f64, bool),
    ContactBegan,
    Wheel(f64, f64),
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
    bindings: Vec<whalu::config::Binding>,
    runtime: *mut Runtime,
    scroll_modifiers: Modifiers,
    wheel: WheelRouter,
    clock: Instant,
    clicks: ui::ThumbnailClicks,
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
    mouse: bool,
    dragging: bool,
    manual_until: Option<Instant>,
    switched_space: bool,
    pending_commands: Vec<Command>,
    space_until: Option<Instant>,
    space_retries: u8,
    paused: bool,
    modal: bool,
    recovery: SavedState,
    restored: BTreeSet<u64>,
    context_tracker: ContextTracker,
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
            let due = [
                scan_due,
                self.overlay_deadline,
                self.space_until,
                self.motion_due,
                self.linger_until,
                self.click_travel.as_ref().map(|travel| travel.due),
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
        while let Ok(event) = self.rx.try_recv() {
            if std::env::var_os("WHALU_TRACE").is_some()
                && matches!(
                    event,
                    Event::SpaceChanged | Event::NativeGesture | Event::Gesture(..)
                )
            {
                eprintln!("Input: {event:?}");
            }
            match event {
                Event::Dirty => self.deadline = Some(Instant::now() + Duration::from_millis(70)),
                Event::SpaceChanged => self.yield_to_system(true),
                Event::NativeGesture => self.yield_to_system(false),
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
                Event::Gesture(device, event, time) => self.gesture_event(device, event, time),
                Event::GestureMotion(device, position, time, sustained) => {
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
                Event::Wheel(delta, time) => self.wheel_event(delta, time),
                Event::SelectThumbnail {
                    context,
                    window,
                    epoch,
                } => {
                    if epoch == self.ui.visibility_epoch() {
                        self.select_thumbnail(context, window);
                    }
                }
                Event::Drag => self.dragging = true,
                Event::Mouse(down) => {
                    if down {
                        self.pending_focus = None;
                        self.clear_scroll();
                        self.render();
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
                            .is_some_and(|w| w.mode == whalu::core::Mode::Magnetic)
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
                    // Join the writer only at shutdown so the final state is durable.
                    self.state_writer.take();
                    self.ui.hide();
                    NSApplication::sharedApplication(self.mtm).terminate(None);
                }
                Event::TapDisabled => {
                    self.clear_scroll();
                    unsafe {
                        let q = QUEUE.load(Ordering::Acquire);
                        if !q.is_null() {
                            (*q).wheel.reset();
                            (*q).clicks.reset();
                        }
                    }
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
        self.advance_scroll(now);
        self.advance_click(now);
        self.confirm_focus(now);
        self.schedule();
    }
    fn confirm_focus(&mut self, now: Instant) {
        let Some(pending) = self
            .pending_focus
            .as_ref()
            .filter(|pending| pending.due <= now)
        else {
            return;
        };
        let target = self.discovery.handles.get(&pending.window);
        let confirmed = target.is_some_and(|handle| {
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|app| app.processIdentifier() == handle.pid)
                && unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
                    .and_then(|app| {
                        unsafe {
                            AXUIElementSetMessagingTimeout(app.0, 0.05);
                        }
                        attr(app.0, "AXFocusedWindow")
                    })
                    .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
        });
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
        // Four-finger contact precedes the workspace notification. Abandon the
        // session before its cancellation event can restore an old-Space window.
        self.clear_scroll();
        self.discovery.invalidate_navigation();
        self.pending_focus = None;
        self.pending_commands.clear();
        self.switched_space |= space_changed;
        self.manual_until = None;
        self.space_until = Some(Instant::now() + Duration::from_millis(850));
        self.space_retries = 8;
        self.engine.frozen = true;
        self.overlay_deadline = None;
        self.ui.reset_overview();
        self.deadline = None;
    }
    fn clear_scroll(&mut self) {
        self.click_travel = None;
        self.gesture = None;
        self.motion_due = None;
        self.linger_until = None;
    }
    fn prepare_command(&mut self, command: Command) {
        let navigation = matches!(
            command,
            Command::FocusNext | Command::FocusPrevious | Command::AppNext | Command::AppPrevious
        );
        if navigation && !self.modal {
            if let Some(observed) = self.discovery.navigation_focus() {
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
                return;
            }
        }
        self.reconcile(false);
        self.deadline = None;
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
        if !self.discovery.is_onscreen(window) {
            self.ui.reset_overview();
            self.deadline = Some(Instant::now());
            return;
        }
        let visual = self
            .click_travel
            .as_ref()
            .and_then(|travel| travel.sequence.selected())
            .or(self.engine.focused());
        let sequence = ClickSequence::from_selection(&self.engine, window, visual);
        self.clear_scroll();
        self.overlay_deadline = None;
        self.apply(vec![Effect::Focus(window)]);
        if self.engine.focused() == Some(window) {
            self.click_travel = sequence.map(|sequence| ClickTravel {
                sequence,
                due: Instant::now(),
            });
            self.advance_click(Instant::now());
            self.deadline = Some(Instant::now() + Duration::from_millis(100));
        } else {
            self.linger_until = Some(Instant::now() + Duration::from_millis(650));
        }
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
        let next = self
            .click_travel
            .as_mut()
            .and_then(|travel| travel.sequence.advance(&self.engine));
        if self.engine.frozen || self.space_until.is_some() {
            self.clear_scroll();
            self.render();
            return;
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
        self.linger_until = Some(Instant::now() + Duration::from_millis(650));
        self.reflow();
        self.render();
        self.save();
    }
    fn contact_began(&mut self) {
        // Resting or repositioning a finger after a click is not navigation.
        // Only a recognized gesture (begin_scroll) interrupts visual click travel.
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
        // A new touch/wheel movement immediately catches an existing coast.
        if self.gesture.as_ref().is_some_and(|active| !active.released) {
            return;
        }
        if self.space_until.is_some() || self.mouse {
            return;
        }
        if self.click_travel.take().is_some() {
            self.linger_until = Some(Instant::now() + Duration::from_millis(650));
        }
        let reuse = self
            .linger_until
            .is_some_and(|until| until > Instant::now())
            && self.deadline.is_none()
            && !self.switched_space;
        self.gesture = None;
        self.motion_due = None;
        // Do not clear feedback before reconciliation: it can render, and a
        // momentary hidden panel would restart all animations between swipes.
        if !reuse {
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
    fn gesture_event(&mut self, device: usize, event: GestureEvent, time: f64) {
        match event {
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
        self.linger_until = Some(Instant::now() + Duration::from_millis(650));
        self.reflow();
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
                self.engine.gap = c.gap;
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
                    (*q).wheel.reset();
                }
                self.clear_scroll();
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
                if live.mode == whalu::core::Mode::Floating {
                    live.last_floating_frame = Some(live.frame);
                }
            }
            self.restored.insert(id);
        }
        let focused = if snap.modal {
            snap.focused
        } else {
            self.pending_focus.as_ref().map_or(snap.focused, |pending| {
                pending.observed(context, &snap.visible, snap.focused, Instant::now())
            })
        };
        if snap.modal
            || context_changed
            || self.pending_focus.as_ref().is_some_and(|pending| {
                snap.focused == Some(pending.window) || !snap.visible.contains(&pending.window)
            })
        {
            self.pending_focus = None;
        }
        self.engine.set_visible(context, &snap.visible, focused);
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
            self.ui.reset_overview();
        }
        self.switched_space = false;
        self.observe();
        let changed = previous != snap.focused;
        if allow_layout && changed && !context_changed && !direct_manipulation {
            self.reflow();
        }
        self.render();
        self.save();
        if self.space_retries > 0 {
            if snap.visible.is_empty() {
                self.space_retries -= 1;
                self.deadline = Some(Instant::now() + Duration::from_millis(120));
            } else {
                self.space_retries = 0;
            }
        }
    }
    fn reflow(&mut self) {
        if !self.paused
            && self.config.magnetic
            && self.gesture.is_none()
            && self.click_travel.is_none()
        {
            self.apply(self.engine.plan());
        }
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        if self.space_until.is_some() || self.switched_space {
            return;
        }
        for effect in effects {
            let id = match effect {
                Effect::Focus(id) | Effect::Move(id, _) | Effect::Snap(id, _) => id,
            };
            let Some(h) = self.discovery.handles.get(&id) else {
                continue;
            };
            // Cached visibility may still describe the previous Space before
            // NSWorkspace delivers its change notification. Never raise or move
            // a window that has already left the screen.
            if !h.visible || !self.discovery.is_onscreen(id) {
                self.pending_focus = None;
                self.deadline = Some(Instant::now());
                continue;
            }
            if std::env::var_os("WHALU_TRACE").is_some() {
                eprintln!("Apply {effect:?}: pid={}, capture={:?}", h.pid, h.quartz_id);
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
                Effect::Move(id, r) => {
                    let Some(w) = self.engine.windows.get(&id) else {
                        continue;
                    };
                    // Enforce M-01 at the native boundary, independently of the solver.
                    if w.mode != whalu::core::Mode::Magnetic
                        || (w.frame.width - r.width).abs() > 0.01
                        || (w.frame.height - r.height).abs() > 0.01
                    {
                        continue;
                    }
                    if let Err(code) = set_position(h.element.0, r) {
                        eprintln!("Window {id}: move rejected (AX {code})");
                    }
                    if let Some(actual) = frame(h.element.0) {
                        self.discovery.observed_frame(id, actual);
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
                        self.discovery.observed_frame(id, actual);
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
        if (keyboard
            || self.gesture.is_some()
            || self.click_travel.is_some()
            || self.linger_until.is_some())
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
            self.ui.hide();
        }
    }
    fn save(&mut self) {
        if let Some(writer) = self.state_writer.as_ref() {
            writer.save(SavedState::capture(&self.engine));
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
            match (*q).wheel.route(
                modifiers.overlay((*q).scroll_modifiers),
                delta,
                continuous,
                momentum,
                CGEventGetIntegerValueField(event, 99),
                now,
            ) {
                WheelRoute::Pass => {}
                WheelRoute::Consume => return null_mut(),
                WheelRoute::Navigate(delta) => {
                    enqueue(Event::Modifiers(modifiers));
                    enqueue(Event::Wheel(delta, now));
                    return null_mut();
                }
            }
        }
    }
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
            "Native Space visibility: {}",
            if spaces::SpaceVisibility::read().is_some() {
                "read-only membership and animation checks"
            } else {
                "Quartz visibility cohort fallback"
            }
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
        let mask = (1 << 22) | (1 << 1) | (1 << 2) | (1 << 6) | (1 << 10) | (1 << 11) | (1 << 12);
        let Some(tap) = onboarding::wait(mtm, mask) else {
            return Ok(());
        };
        let (tx, rx) = mpsc::channel();
        let mut queue = Box::new(Queue {
            tx,
            source: null(),
            bindings: config.bindings.clone(),
            runtime: null_mut(),
            scroll_modifiers: config.overlay_modifiers,
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
            state_writer: Some(
                StateWriter::new(config_dir().join("state.json")).map_err(|e| e.to_string())?,
            ),
            discovery: Discovery::new(),
            ui,
            observers: BTreeMap::new(),
            gesture: None,
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
            mouse: false,
            dragging: false,
            manual_until: None,
            switched_space: false,
            pending_commands: vec![],
            space_until: None,
            space_retries: 0,
            paused: false,
            modal: false,
            recovery: SavedState::load(&config_dir().join("state.json")),
            restored: BTreeSet::new(),
            context_tracker: ContextTracker::default(),
            _lock: lock,
        });
        queue.runtime = &mut *runtime;
        *DISPATCH.lock().unwrap() = Some((queue.tx.clone(), source.0 as usize));
        QUEUE.store(ptr, Ordering::Release);
        runtime.ui.gesture_status(runtime.gestures.is_some());
        runtime.reconcile(false);
        eprintln!(
            "Whalu running. Hold Control + Option to navigate; use the W menu to pause or quit."
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
