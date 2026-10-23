//! Small C ABI surface. Create/Copy results are immediately wrapped in `Owned`.
#![allow(unsafe_op_in_unsafe_fn)]
use std::ffi::{c_char, c_void};
pub type Ref = *const c_void;
pub type MutRef = *mut c_void;
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct CGRect {
    pub origin: Point,
    pub size: Size,
}
#[repr(C)]
pub struct SourceContext {
    pub version: isize,
    pub info: MutRef,
    pub retain: Option<unsafe extern "C" fn(Ref) -> Ref>,
    pub release: Option<unsafe extern "C" fn(Ref)>,
    pub description: Option<unsafe extern "C" fn(Ref) -> Ref>,
    pub equal: Option<unsafe extern "C" fn(Ref, Ref) -> bool>,
    pub hash: Option<unsafe extern "C" fn(Ref) -> usize>,
    pub schedule: Option<unsafe extern "C" fn(MutRef, Ref, Ref)>,
    pub cancel: Option<unsafe extern "C" fn(MutRef, Ref, Ref)>,
    pub perform: Option<unsafe extern "C" fn(MutRef)>,
}
#[repr(C)]
pub struct TimerContext {
    pub version: isize,
    pub info: MutRef,
    pub retain: Option<unsafe extern "C" fn(Ref) -> Ref>,
    pub release: Option<unsafe extern "C" fn(Ref)>,
    pub description: Option<unsafe extern "C" fn(Ref) -> Ref>,
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFRetain(value: Ref) -> Ref;
    pub fn CFRelease(value: Ref);
    pub fn CFEqual(a: Ref, b: Ref) -> bool;
    pub fn CFGetTypeID(v: Ref) -> usize;
    pub fn CFStringGetTypeID() -> usize;
    pub fn CFBooleanGetTypeID() -> usize;
    pub fn CFArrayGetTypeID() -> usize;
    pub fn CFStringCreateWithCString(a: Ref, s: *const c_char, encoding: u32) -> Ref;
    pub fn CFStringGetLength(s: Ref) -> isize;
    pub fn CFStringGetMaximumSizeForEncoding(len: isize, encoding: u32) -> isize;
    pub fn CFStringGetCString(s: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
    pub fn CFBooleanGetValue(value: Ref) -> bool;
    pub fn CFArrayGetCount(a: Ref) -> isize;
    pub fn CFArrayGetValueAtIndex(a: Ref, i: isize) -> Ref;
    pub fn CFDictionaryGetValue(d: Ref, key: Ref) -> Ref;
    pub fn CFDictionaryCreate(
        a: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: Ref,
        value_callbacks: Ref,
    ) -> Ref;
    pub fn CFNumberGetValue(n: Ref, ty: isize, out: MutRef) -> bool;
    pub fn CFRunLoopGetMain() -> Ref;
    pub fn CFRunLoopRunInMode(mode: Ref, seconds: f64, return_after_source: bool) -> i32;
    pub static kCFRunLoopDefaultMode: Ref;
    pub fn CFRunLoopAddSource(runloop: Ref, source: Ref, mode: Ref);
    pub fn CFRunLoopRemoveSource(runloop: Ref, source: Ref, mode: Ref);
    pub fn CFRunLoopSourceCreate(a: Ref, order: isize, context: *mut SourceContext) -> Ref;
    pub fn CFRunLoopSourceSignal(source: Ref);
    pub fn CFRunLoopWakeUp(runloop: Ref);
    pub fn CFMachPortCreateRunLoopSource(a: Ref, port: Ref, order: isize) -> Ref;
    pub fn CFAbsoluteTimeGetCurrent() -> f64;
    pub fn CFRunLoopTimerCreate(
        a: Ref,
        date: f64,
        interval: f64,
        flags: usize,
        order: isize,
        callback: unsafe extern "C" fn(Ref, MutRef),
        context: *mut TimerContext,
    ) -> Ref;
    pub fn CFRunLoopAddTimer(runloop: Ref, timer: Ref, mode: Ref);
    pub fn CFRunLoopTimerInvalidate(timer: Ref);
    pub static kCFRunLoopCommonModes: Ref;
    pub static kCFBooleanTrue: Ref;
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: Ref) -> bool;
    pub static kAXTrustedCheckOptionPrompt: Ref;
    pub fn AXUIElementCreateApplication(pid: i32) -> Ref;
    pub fn AXUIElementCopyAttributeValue(element: Ref, attr: Ref, value: *mut Ref) -> i32;
    pub fn AXUIElementSetAttributeValue(element: Ref, attr: Ref, value: Ref) -> i32;
    pub fn AXUIElementIsAttributeSettable(element: Ref, attr: Ref, value: *mut bool) -> i32;
    pub fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
    pub fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    pub fn AXValueCreate(ty: u32, value: Ref) -> Ref;
    pub fn AXValueGetValue(value: Ref, ty: u32, out: MutRef) -> bool;
    pub fn AXValueGetTypeID() -> usize;
    pub fn AXObserverCreate(
        pid: i32,
        callback: unsafe extern "C" fn(Ref, Ref, Ref, MutRef),
        observer: *mut Ref,
    ) -> i32;
    pub fn AXObserverAddNotification(
        observer: Ref,
        element: Ref,
        notification: Ref,
        info: MutRef,
    ) -> i32;
    pub fn AXObserverGetRunLoopSource(observer: Ref) -> Ref;
}
pub type EventCallback = unsafe extern "C" fn(MutRef, u32, MutRef, MutRef) -> MutRef;
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGEventTapCreate(
        location: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: EventCallback,
        info: MutRef,
    ) -> Ref;
    pub fn CGEventTapEnable(tap: Ref, enable: bool);
    pub fn CGEventGetFlags(event: Ref) -> u64;
    pub fn CGEventGetLocation(event: Ref) -> Point;
    pub fn CGEventGetDoubleValueField(event: Ref, field: u32) -> f64;
    pub fn CGEventGetIntegerValueField(event: Ref, field: u32) -> i64;
    pub fn CGPreflightScreenCaptureAccess() -> bool;
    pub fn CGRequestScreenCaptureAccess() -> bool;
    pub fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> Ref;
    pub fn CGRectMakeWithDictionaryRepresentation(dict: Ref, rect: *mut CGRect) -> bool;
    pub fn CGMainDisplayID() -> u32;
    pub fn CGDisplayBounds(id: u32) -> CGRect;
}
#[derive(Debug)]
pub struct Owned(pub Ref);
impl Owned {
    pub unsafe fn new(p: Ref) -> Option<Self> {
        (!p.is_null()).then_some(Self(p))
    }
}
impl Clone for Owned {
    fn clone(&self) -> Self {
        unsafe { Self(CFRetain(self.0)) }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
pub fn string(s: &str) -> Owned {
    let s = std::ffi::CString::new(s).expect("constant without NUL");
    unsafe {
        Owned(CFStringCreateWithCString(
            std::ptr::null(),
            s.as_ptr(),
            0x08000100,
        ))
    }
}
pub unsafe fn text(v: Ref) -> Option<String> {
    if v.is_null() || CFGetTypeID(v) != CFStringGetTypeID() {
        return None;
    }
    let n = CFStringGetMaximumSizeForEncoding(CFStringGetLength(v), 0x08000100) + 1;
    let mut b = vec![0u8; n as usize];
    if !CFStringGetCString(v, b.as_mut_ptr().cast(), n, 0x08000100) {
        return None;
    }
    let len = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    Some(String::from_utf8_lossy(&b[..len]).into_owned())
}
pub fn attr(e: Ref, key: &str) -> Option<Owned> {
    unsafe {
        let mut out = std::ptr::null();
        (AXUIElementCopyAttributeValue(e, string(key).0, &mut out) == 0)
            .then(|| Owned::new(out))
            .flatten()
    }
}
pub fn bool_attr(e: Ref, key: &str) -> bool {
    attr(e, key).is_some_and(|v| unsafe {
        CFGetTypeID(v.0) == CFBooleanGetTypeID() && CFBooleanGetValue(v.0)
    })
}
pub fn text_attr(e: Ref, key: &str) -> String {
    attr(e, key)
        .and_then(|v| unsafe { text(v.0) })
        .unwrap_or_default()
}
pub unsafe fn array(a: Ref) -> Vec<Ref> {
    if a.is_null() || CFGetTypeID(a) != CFArrayGetTypeID() {
        return vec![];
    }
    (0..CFArrayGetCount(a))
        .map(|i| CFArrayGetValueAtIndex(a, i))
        .collect()
}
pub unsafe fn dict(d: Ref, key: &str) -> Ref {
    CFDictionaryGetValue(d, string(key).0)
}
pub unsafe fn number(v: Ref) -> i64 {
    if v.is_null() {
        return 0;
    }
    let mut n = 0i64;
    CFNumberGetValue(v, 4, (&mut n as *mut i64).cast());
    n
}
pub fn frame(e: Ref) -> Option<whalu::geometry::Rect> {
    let p = attr(e, "AXPosition")?;
    let s = attr(e, "AXSize")?;
    let mut point = Point::default();
    let mut size = Size::default();
    unsafe {
        if CFGetTypeID(p.0) != AXValueGetTypeID()
            || CFGetTypeID(s.0) != AXValueGetTypeID()
            || !AXValueGetValue(p.0, 1, (&mut point as *mut Point).cast())
            || !AXValueGetValue(s.0, 2, (&mut size as *mut Size).cast())
        {
            return None;
        }
    }
    let r = whalu::geometry::Rect::new(point.x, point.y, size.width, size.height);
    r.valid().then_some(r)
}
pub fn set_position(e: Ref, r: whalu::geometry::Rect) -> Result<(), i32> {
    unsafe {
        let p = Point { x: r.x, y: r.y };
        let v = Owned::new(AXValueCreate(1, (&p as *const Point).cast())).ok_or(-1)?;
        let code = AXUIElementSetAttributeValue(e, string("AXPosition").0, v.0);
        if code == 0 { Ok(()) } else { Err(code) }
    }
}
pub fn set_size(e: Ref, r: whalu::geometry::Rect) -> Result<(), i32> {
    unsafe {
        let s = Size {
            width: r.width,
            height: r.height,
        };
        let v = Owned::new(AXValueCreate(2, (&s as *const Size).cast())).ok_or(-1)?;
        let code = AXUIElementSetAttributeValue(e, string("AXSize").0, v.0);
        if code == 0 { Ok(()) } else { Err(code) }
    }
}
pub fn settable(e: Ref, key: &str) -> bool {
    unsafe {
        let mut value = false;
        AXUIElementIsAttributeSettable(e, string(key).0, &mut value) == 0 && value
    }
}

/// Reapply size after moving, then anchor the size actually accepted by the app.
pub fn snap_frame(
    element: Ref,
    target: whalu::geometry::Rect,
    bounds: whalu::geometry::Rect,
) -> Result<(), i32> {
    let _ = set_size(element, target);
    let _ = set_position(element, target);
    let resized = set_size(element, target);
    // Even a rejected resize must not leave the old width hanging off-screen.
    let actual = frame(element).ok_or(-25202)?;
    let aligned = whalu::geometry::constrained_snap(target, actual, bounds);
    set_position(element, aligned)?;
    resized
}
