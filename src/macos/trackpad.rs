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
    let (events, progress, fresh, sustained) = RECOGNIZERS
        .lock()
        .map(|mut states| {
            let state = states.entry(device as usize).or_default();
            let fresh = state.contacts == 0 && !contacts.is_empty();
            state.contacts = contacts.len();
            let events = state.recognizer.feed(time, &contacts);
            state.guard.contacts(
                contacts.len(),
                state.recognizer.eligible(),
                state.recognizer.active(),
                clock(),
            );
            let progress = (contacts.len() == 3)
                .then(|| state.recognizer.progress())
                .flatten();
            (events, progress, fresh, state.recognizer.sustained())
        })
        .unwrap_or_default();
    if fresh {
        super::enqueue(super::Event::ContactBegan);
    }
    for event in events {
        if matches!(event, GestureEvent::Begin(_)) {
            SWIPES.fetch_add(1, Ordering::Relaxed);
        }
        super::enqueue(super::Event::Gesture(device as usize, event, time));
    }
    if let Some(progress) = progress {
        super::enqueue(super::Event::GestureMotion(
            device as usize,
            progress,
            time,
            sustained,
        ));
    }
}

fn cancel_device(device: Ref) {
    if let Ok(mut states) = RECOGNIZERS.lock() {
        if let Some(state) = states.get_mut(&(device as usize)) {
            state.guard = ScrollGuard::default();
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
