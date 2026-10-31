//! A narrow HID tap precedes mouse-remapper actions without touching scrolling.
use super::{Event, ffi::*, ui::ThumbnailClicks};
use std::{ptr::null, sync::mpsc::Sender};
use whalu::input::{ButtonRoute, OverviewButtons};

struct Context {
    clicks: ThumbnailClicks,
    buttons: OverviewButtons,
    tx: Sender<Event>,
    wake_source: Ref,
    port: Ref,
}
impl Context {
    fn route(&mut self, kind: u32, button: i64) -> bool {
        let target = self.clicks.navigation_target();
        match self.buttons.route(target.is_some(), kind, button) {
            ButtonRoute::Pass => false,
            ButtonRoute::Consume(command) => {
                if let (Some(command), Some((context, epoch))) = (command, target) {
                    let _ = self.tx.send(Event::OverviewButton {
                        context,
                        epoch,
                        command,
                    });
                    if !self.wake_source.is_null() {
                        unsafe {
                            CFRunLoopSourceSignal(self.wake_source);
                            CFRunLoopWakeUp(CFRunLoopGetMain());
                        }
                    }
                }
                true
            }
        }
    }
}

pub(super) struct Tap {
    context: Box<Context>,
    port: Owned,
    source: Owned,
}
impl Tap {
    pub fn start(
        clicks: ThumbnailClicks,
        tx: Sender<Event>,
        wake_source: Ref,
    ) -> Result<Self, String> {
        let mut context = Box::new(Context {
            clicks,
            buttons: OverviewButtons::default(),
            tx,
            wake_source,
            port: null(),
        });
        unsafe {
            // HID, head insertion: own visible-overview side clicks before
            // tools such as LinearMouse turn them into browser/Space actions.
            let port = Owned::new(CGEventTapCreate(
                0,
                0,
                0,
                (1 << 25) | (1 << 26) | (1 << 27),
                callback,
                (&mut *context as *mut Context).cast(),
            ))
            .ok_or("Cannot create overview mouse-button event tap")?;
            context.port = port.0;
            let source = Owned::new(CFMachPortCreateRunLoopSource(null(), port.0, 0))
                .ok_or("Cannot create overview mouse-button run-loop source")?;
            CFRunLoopAddSource(CFRunLoopGetMain(), source.0, kCFRunLoopCommonModes);
            CGEventTapEnable(port.0, true);
            Ok(Self {
                context,
                port,
                source,
            })
        }
    }

    /// Exercise the same callback with fixture events, without posting input.
    pub(super) fn verify_event(&mut self, kind: u32, event: MutRef) -> bool {
        unsafe {
            callback(
                std::ptr::null_mut(),
                kind,
                event,
                (&mut *self.context as *mut Context).cast(),
            )
            .is_null()
        }
    }
}
impl Drop for Tap {
    fn drop(&mut self) {
        unsafe {
            CGEventTapEnable(self.port.0, false);
            CFRunLoopRemoveSource(CFRunLoopGetMain(), self.source.0, kCFRunLoopCommonModes);
            CFMachPortInvalidate(self.port.0);
        }
    }
}

unsafe extern "C" fn callback(_: MutRef, kind: u32, event: MutRef, info: MutRef) -> MutRef {
    let context = unsafe { &mut *info.cast::<Context>() };
    if matches!(kind, 0xffff_fffe | 0xffff_ffff) {
        unsafe {
            CGEventTapEnable(context.port, true);
        }
        return event;
    }
    if context.route(kind, unsafe { CGEventGetIntegerValueField(event, 3) }) {
        std::ptr::null_mut()
    } else {
        event
    }
}
