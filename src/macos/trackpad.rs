//! Read-only contact detection plus ownership of three-finger scroll streams.
//! Native scroll suppression happens in the public CGEvent tap; settings are unchanged.
//! ABI reference: https://github.com/asmagill/hs._asm.undocumented.touchdevice/blob/master/MultitouchSupport.h
#![allow(unsafe_op_in_unsafe_fn)]
use super::ffi::*;
use std::{
    collections::BTreeMap,
    ffi::CStr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use whalu::{
    gestures::{Contact, GestureEvent, SwipeRecognizer},
    scroll::ScrollGuard,
};
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Vector {
    position: Point,
    velocity: Point,
}
#[repr(C)]
struct Touch {
    frame: i32,
    time: f64,
    path: i32,
    state: i32,
    finger: i32,
    hand: i32,
    normalized: Vector,
    quality: f32,
    pressure: f32,
    angle: f32,
    major: f32,
    minor: f32,
    absolute: Vector,
    field14: i32,
    field15: i32,
    density: f32,
}
type Callback = unsafe extern "C" fn(Ref, *const Touch, usize, f64, usize, MutRef);
type Register = unsafe extern "C" fn(Ref, Callback) -> i8;
type Start = unsafe extern "C" fn(Ref, i32) -> i32;
type Stop = unsafe extern "C" fn(Ref) -> i32;
#[derive(Default)]
struct Tracked {
    recognizer: SwipeRecognizer,
    guard: ScrollGuard,
    contacts: usize,
    overview: bool,
}
impl Tracked {
    fn feed(&mut self, device: usize, time: f64, contacts: &[Contact]) -> Vec<super::Event> {
        let fresh = self.contacts == 0 && !contacts.is_empty();
        let native_gesture = self.contacts < 4 && contacts.len() >= 4;
        self.contacts = contacts.len();
        let gestures = self.recognizer.feed(time, contacts);
        self.guard.contacts(
            contacts.len(),
            self.recognizer.eligible(),
            self.recognizer.active(),
            clock(),
        );
        let mut events = vec![];
        if fresh {
            events.push(super::Event::ContactBegan);
        }
        // This must precede the recognizer's cancelled End: restoring the old
        // selection during a four-finger Space swipe can pull macOS back.
        if native_gesture {
            events.push(super::Event::NativeGesture);
        }
        let overview = contacts.len() == 3 && self.recognizer.overview_ready();
        if overview && !self.overview {
            events.push(super::Event::TouchOverview(device, true));
        }
        events.extend(
            gestures
                .into_iter()
                .map(|event| super::Event::Gesture(device, event, time)),
        );
        if let Some(progress) = (contacts.len() == 3)
            .then(|| self.recognizer.progress())
            .flatten()
        {
            events.push(super::Event::GestureMotion(
                device,
                progress,
                time,
                self.recognizer.sustained(),
            ));
        }
        if !overview && self.overview {
            events.push(super::Event::TouchOverview(device, false));
        }
        self.overview = overview;
        events
    }
}
static RECOGNIZERS: Mutex<BTreeMap<usize, Tracked>> = Mutex::new(BTreeMap::new());
fn clock() -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64()
}
pub fn suppress_scroll(continuous: bool, momentum: i64) -> bool {
    RECOGNIZERS.lock().is_ok_and(|mut states| {
        states
            .values_mut()
            .any(|state| state.guard.capture(continuous, momentum, clock()))
    })
}
static FRAMES: AtomicU64 = AtomicU64::new(0);
static INVALID: AtomicU64 = AtomicU64::new(0);
static SWIPES: AtomicU64 = AtomicU64::new(0);
unsafe extern "C" fn callback(
    device: Ref,
    touches: *const Touch,
    count: usize,
    time: f64,
    _frame: usize,
    _info: MutRef,
) {
    FRAMES.fetch_add(1, Ordering::Relaxed);
    if count > 32 || (count > 0 && touches.is_null()) {
        INVALID.fetch_add(1, Ordering::Relaxed);
        cancel_device(device);
        return;
    }
    let data = if count == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(touches, count)
    };
    let contacts: Vec<_> = data
        .iter()
        .filter(|t| t.state == 3 || t.state == 4)
        .map(|t| Contact {
            id: t.path,
            x: t.normalized.position.x as f64,
            y: t.normalized.position.y as f64,
        })
        .collect();
    if data.iter().any(|t| !(0..=7).contains(&t.state))
        || contacts.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(0. ..=1.).contains(&p.x)
                || !(0. ..=1.).contains(&p.y)
        })
    {
        INVALID.fetch_add(1, Ordering::Relaxed);
        cancel_device(device);
        return;
    }
    let events = RECOGNIZERS
        .lock()
        .map(|mut states| {
            let state = states.entry(device as usize).or_default();
            state.feed(device as usize, time, &contacts)
        })
        .unwrap_or_default();
    for event in events {
        if matches!(event, super::Event::Gesture(_, GestureEvent::Begin(_), _)) {
            SWIPES.fetch_add(1, Ordering::Relaxed);
        }
        super::enqueue(event);
    }
}

fn cancel_device(device: Ref) {
    if let Ok(mut states) = RECOGNIZERS.lock() {
        if let Some(state) = states.get_mut(&(device as usize)) {
            state.guard = ScrollGuard::default();
            if std::mem::take(&mut state.overview) {
                super::enqueue(super::Event::TouchOverview(device as usize, false));
            }
            for event in state.recognizer.cancel() {
                super::enqueue(super::Event::Gesture(device as usize, event, 0.));
            }
        }
    }
}
pub struct Monitor {
    library: MutRef,
    _devices: Owned,
    register: Register,
    started: Vec<Ref>,
    stop: Stop,
    pub count: usize,
}
impl Monitor {
    pub fn start() -> Result<Self, String> {
        unsafe {
            let library = libc::dlopen(
                c"/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport"
                    .as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            );
            if library.is_null() {
                return Err("Trackpad framework is unavailable".into());
            }
            unsafe fn symbol(library: MutRef, name: &CStr) -> Result<MutRef, String> {
                let value = libc::dlsym(library, name.as_ptr());
                if value.is_null() {
                    Err(format!(
                        "Trackpad function {} is unavailable",
                        name.to_string_lossy()
                    ))
                } else {
                    Ok(value)
                }
            }
            // Keep the image loaded for process lifetime: callbacks may be completing on
            // the framework's thread while a monitor is stopped/reloaded.
            let create: unsafe extern "C" fn() -> Ref =
                std::mem::transmute(symbol(library, c"MTDeviceCreateList")?);
            let register: Register =
                std::mem::transmute(symbol(library, c"MTRegisterContactFrameCallback")?);
            let unregister: Register =
                std::mem::transmute(symbol(library, c"MTUnregisterContactFrameCallback")?);
            let start: Start = std::mem::transmute(symbol(library, c"MTDeviceStart")?);
            let stop: Stop = std::mem::transmute(symbol(library, c"MTDeviceStop")?);
            let built_in: unsafe extern "C" fn(Ref) -> i8 =
                std::mem::transmute(symbol(library, c"MTDeviceIsBuiltIn")?);
            let force: unsafe extern "C" fn(Ref) -> i8 =
                std::mem::transmute(symbol(library, c"MTDeviceSupportsForce")?);
            let devices = Owned::new(create()).ok_or("No trackpad device list")?;
            if let Ok(mut states) = RECOGNIZERS.lock() {
                states.clear();
            }
            let mut count = 0;
            let mut started = vec![];
            for device in array(devices.0) {
                if built_in(device) == 0 && force(device) == 0 {
                    continue;
                }
                if register(device, callback) != 0 {
                    if start(device, 0) == 0 {
                        count += 1;
                        started.push(device);
                    } else {
                        unregister(device, callback);
                    }
                }
            }
            if count == 0 {
                return Err("No readable trackpad is connected".into());
            }
            Ok(Self {
                library,
                _devices: devices,
                register: unregister,
                started,
                stop,
                count,
            })
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            for device in &self.started {
                (self.register)(*device, callback);
                (self.stop)(*device);
            }
            if let Ok(mut states) = RECOGNIZERS.lock() {
                states.clear();
            }
            // No dlclose: see lifecycle note above. The OS releases the image on exit.
            let _ = self.library;
        }
    }
}
pub fn stats() -> (u64, u64, u64) {
    (
        FRAMES.load(Ordering::Relaxed),
        SWIPES.load(Ordering::Relaxed),
        INVALID.load(Ordering::Relaxed),
    )
}
pub fn probe(seconds: f64) -> Result<(), String> {
    let _monitor = Monitor::start()?;
    println!(
        "Listening on {} trackpad(s) for {seconds:.0}s. Swipe with three fingers. No navigation commands run in this probe.",
        _monitor.count
    );
    let end = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
    while std::time::Instant::now() < end {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, false);
        }
    }
    let (frames, swipes, invalid) = stats();
    println!(
        "Contact frames: {frames}; three-finger swipes: {swipes}; rejected ABI frames: {invalid}"
    );
    Ok(())
}
const _: () = assert!(std::mem::size_of::<Touch>() == 96);

#[cfg(test)]
mod tests {
    use super::*;

    fn contacts(count: i32, x: f64) -> Vec<Contact> {
        (0..count)
            .map(|id| Contact {
                id,
                x: x + id as f64 * 0.05,
                y: 0.5,
            })
            .collect()
    }

    #[test]
    fn stationary_contact_opens_once_and_release_closes_without_navigation() {
        let mut tracked = Tracked::default();
        tracked.feed(9, 0., &contacts(3, 0.3));
        assert!(tracked.feed(9, 0.02, &contacts(3, 0.3)).is_empty());
        let open = tracked.feed(9, 0.05, &contacts(3, 0.3));
        assert!(matches!(
            open.as_slice(),
            [super::super::Event::TouchOverview(9, true)]
        ));
        assert!(tracked.feed(9, 1., &contacts(3, 0.3)).is_empty());
        assert!(matches!(
            tracked.feed(9, 1.1, &[]).as_slice(),
            [super::super::Event::TouchOverview(9, false)]
        ));
    }

    #[test]
    fn four_finger_landing_never_opens_contact_overview() {
        let mut tracked = Tracked::default();
        tracked.feed(9, 0., &contacts(3, 0.3));
        tracked.feed(9, 0.02, &contacts(3, 0.3));
        let native = tracked.feed(9, 0.04, &contacts(4, 0.3));
        assert!(matches!(
            native.as_slice(),
            [super::super::Event::NativeGesture]
        ));
        assert!(tracked.feed(9, 0.1, &contacts(3, 0.3)).is_empty());
    }

    #[test]
    fn fourth_contact_yields_before_cancelling_a_recognized_three_finger_swipe() {
        let mut tracked = Tracked::default();
        tracked.feed(1, 0., &contacts(3, 0.3));
        tracked.feed(1, 0.04, &contacts(3, 0.32));
        let began = tracked.feed(1, 0.09, &contacts(3, 0.4));
        assert!(began.iter().any(|event| matches!(
            event,
            super::super::Event::Gesture(_, GestureEvent::Begin(_), _)
        )));
        let events = tracked.feed(1, 0.1, &contacts(4, 0.41));
        assert!(matches!(events[0], super::super::Event::NativeGesture));
        assert!(matches!(
            events[1],
            super::super::Event::Gesture(_, GestureEvent::End { cancelled: true }, _)
        ));
        assert!(tracked.feed(1, 0.11, &contacts(4, 0.42)).is_empty());
        assert!(tracked.feed(1, 0.12, &contacts(3, 0.43)).is_empty());
        tracked.feed(1, 0.13, &[]);
        let fresh = tracked.feed(1, 0.2, &contacts(3, 0.3));
        assert!(matches!(fresh[0], super::super::Event::ContactBegan));
        assert!(
            !fresh
                .iter()
                .any(|event| matches!(event, super::super::Event::NativeGesture))
        );
    }

    #[test]
    fn direct_four_finger_landing_yields_without_a_whalu_session() {
        let mut tracked = Tracked::default();
        let events = tracked.feed(1, 0., &contacts(4, 0.3));
        assert!(matches!(events[0], super::super::Event::ContactBegan));
        assert!(matches!(events[1], super::super::Event::NativeGesture));
        assert_eq!(events.len(), 2);
    }
}
