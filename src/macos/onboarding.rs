//! Keep a visible, recoverable app while macOS permissions are missing.
use super::ffi::*;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::{
    NSDate, NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::Cell,
    ptr::null,
    time::{Duration, Instant},
};

thread_local! {
    static CANCELLED: Cell<bool> = const { Cell::new(false) };
    static KEYBOARD: Cell<bool> = const { Cell::new(false) };
}

define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    struct SetupDelegate;
    unsafe impl NSObjectProtocol for SetupDelegate {}
    impl SetupDelegate {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self,_sender:Option<&NSObject>) {
            let keyboard=KEYBOARD.get();
            if !keyboard {request_accessibility();}
            let url=if keyboard {"x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"} else {"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"};
            let _=std::process::Command::new("/usr/bin/open").arg(url).spawn();
        }
        #[unsafe(method(quit:))]
        fn quit(&self,_sender:Option<&NSObject>) {CANCELLED.set(true);}
    }
);

fn request_accessibility() {
    unsafe {
        let key = kAXTrustedCheckOptionPrompt;
        let value = kCFBooleanTrue;
        if let Some(options) =
            Owned::new(CFDictionaryCreate(null(), &key, &value, 1, null(), null()))
        {
            AXIsProcessTrustedWithOptions(options.0);
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Issue {
    Accessibility,
    Keyboard,
}
fn check(mask: u64) -> Result<Owned, Issue> {
    unsafe {
        if !AXIsProcessTrusted() {
            return Err(Issue::Accessibility);
        }
        Owned::new(CGEventTapCreate(
            1,
            0,
            0,
            mask,
            super::event_callback,
            std::ptr::null_mut(),
        ))
        .ok_or(Issue::Keyboard)
    }
}

/// No window enumeration or interception until access is ready. Only permission
/// checks repeat while this setup screen is visible; normal runtime remains event-driven.
pub fn wait(mtm: MainThreadMarker, mask: u64) -> Option<Owned> {
    let mut issue = match check(mask) {
        Ok(tap) => return Some(tap),
        Err(issue) => issue,
    };
    CANCELLED.set(false);
    unsafe {
        let delegate: Retained<SetupDelegate> = msg_send![SetupDelegate::alloc(mtm), init];
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(520., 330.)),
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window.setTitle(&NSString::from_str("Set up Whalu"));
        let icon = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            NSRect::new(NSPoint::new(24., 264.), NSSize::new(48., 48.)),
        );
        icon.setImage(NSImage::imageNamed(&NSString::from_str("NSApplicationIcon")).as_deref());
        icon.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        let heading = NSTextField::labelWithString(
            &NSString::from_str("Whalu is waiting for permission"),
            mtm,
        );
        heading.setFont(Some(&NSFont::boldSystemFontOfSize(19.)));
        heading.setFrame(NSRect::new(NSPoint::new(84., 275.), NSSize::new(412., 30.)));
        let message = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
        message.setFont(Some(&NSFont::systemFontOfSize(14.)));
        message.setFrame(NSRect::new(NSPoint::new(24., 85.), NSSize::new(472., 175.)));
        let open = NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Open Accessibility Settings"),
            Some(&delegate),
            Some(sel!(openSettings:)),
            mtm,
        );
        open.setFrame(NSRect::new(NSPoint::new(230., 24.), NSSize::new(266., 34.)));
        let quit = NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Quit"),
            Some(&delegate),
            Some(sel!(quit:)),
            mtm,
        );
        quit.setFrame(NSRect::new(NSPoint::new(24., 24.), NSSize::new(85., 34.)));
        let content = window.contentView().unwrap();
        for view in [
            &*icon as &NSView,
            &*heading as &NSView,
            &*message as &NSView,
            &*open as &NSView,
            &*quit as &NSView,
        ] {
            content.addSubview(view);
        }
        let status = NSStatusBar::systemStatusBar().statusItemWithLength(-1.);
        if let Some(button) = status.button(mtm) {
            button.setTitle(&NSString::from_str("W •"));
            button.setToolTip(Some(&NSString::from_str("Whalu: permission required")));
        }
        let menu = NSMenu::new(mtm);
        let title = NSMenuItem::new(mtm);
        title.setTitle(&NSString::from_str("Whalu needs permission"));
        title.setEnabled(false);
        menu.addItem(&title);
        for (name, action) in [
            ("Open permission settings…", sel!(openSettings:)),
            ("Quit Whalu", sel!(quit:)),
        ] {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(name),
                Some(action),
                &NSString::from_str(""),
            );
            item.setTarget(Some(&delegate));
            menu.addItem(&item);
        }
        status.setMenu(Some(&menu));
        let update = |issue: Issue| {
            KEYBOARD.set(issue == Issue::Keyboard);
            let (title, text) = match issue {
                Issue::Accessibility => (
                    "Open Accessibility Settings",
                    "Whalu needs Accessibility access to focus and arrange other apps’ windows.\n\nEnable Whalu in Privacy & Security → Accessibility. If it is missing, use + to add ~/Applications/Whalu.app.\n\nLeave this window open. Whalu starts automatically when access is granted.",
                ),
                Issue::Keyboard => (
                    "Open Input Monitoring Settings",
                    "Accessibility is enabled, but macOS has not allowed the keyboard event tap.\n\nEnable Whalu in Privacy & Security → Input Monitoring. If it is missing, use + to add ~/Applications/Whalu.app.\n\nLeave this window open. Whalu retries automatically.",
                ),
            };
            open.setTitle(&NSString::from_str(title));
            message.setStringValue(&NSString::from_str(text));
        };
        update(issue);
        window.center();
        window.makeKeyAndOrderFront(None);
        eprintln!(
            "Whalu waiting for {} permission; the W • menu remains available.",
            if issue == Issue::Accessibility {
                "Accessibility"
            } else {
                "keyboard"
            }
        );
        let mut next = Instant::now() + Duration::from_secs(1);
        let result = loop {
            objc2::rc::autoreleasepool(|_| {
                let app = NSApplication::sharedApplication(mtm);
                if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&NSDate::dateWithTimeIntervalSinceNow(0.2)),
                    NSDefaultRunLoopMode,
                    true,
                ) {
                    app.sendEvent(&event);
                }
                app.updateWindows();
            });
            if CANCELLED.get() {
                break None;
            }
            if Instant::now() < next {
                continue;
            }
            next = Instant::now() + Duration::from_secs(1);
            match check(mask) {
                Ok(tap) => break Some(tap),
                Err(current) => {
                    if current != issue {
                        issue = current;
                        update(issue);
                    }
                }
            }
        };
        window.orderOut(None);
        if result.is_some() {
            NSApplication::sharedApplication(mtm).hide(None);
        }
        NSStatusBar::systemStatusBar().removeStatusItem(&status);
        result
    }
}
