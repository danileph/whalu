//! Opt-in diagnostics. Only fixture windows owned by a child process are manipulated.
use super::{discovery::Discovery, ffi::*, ui::Ui};
use magnetic::{
    core::{Command, Effect, Engine, Mode, Window},
    geometry::{Direction, Rect},
    gestures::Axis,
    navigation::ScrubSession,
};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::process::{Child, Command as ProcessCommand};

pub fn preview(path: &str, hold: bool) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    let mut e = Engine::new(8.);
    for (id, name, title, x) in [
        (1, "Codex", "Window navigation", 0.),
        (2, "Telegram", "Chats", 200.),
        (
            3,
            "Terminal",
            "Continuous individual-window navigation",
            800.,
        ),
        (4, "Notes", "A quieter desktop", 300.),
    ] {
        e.windows.insert(
            id,
            Window {
                id,
                app_id: name.into(),
                app_name: name.into(),
                title: title.into(),
                display: 1,
                frame: Rect::new(x, 60., 650., 700.),
                mode: if id == 4 {
                    Mode::Floating
                } else {
                    Mode::Magnetic
                },
                last_floating_frame: None,
                snap: None,
                restore_frame: None,
            },
        );
    }
    e.set_visible(1, &[1, 2, 3, 4], Some(3));
    let ui = Ui::new(mtm);
    ui.show(&e);
    let result = ui.snapshot(path);
    if hold {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
        while std::time::Instant::now() < until {
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
            }
        }
    }
    ui.hide();
    result
}

pub fn fixture() -> Result<(), String> {
    fixture_with_minimum(false)
}
pub fn constrained_fixture() -> Result<(), String> {
    fixture_with_minimum(true)
}

fn fixture_with_minimum(constrained: bool) -> Result<(), String> {
    unsafe {
        let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let mut windows = vec![];
        let titles = ["Magnetic fixture A", "Magnetic fixture B"];
        for (i, title) in titles.iter().enumerate() {
            let w = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(
                    NSPoint::new(130. + i as f64 * 240., 180.),
                    NSSize::new(400., 300.),
                ),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            );
            if constrained {
                let b = NSScreen::mainScreen(mtm)
                    .ok_or("No fixture screen")?
                    .visibleFrame();
                w.setMinSize(NSSize::new(b.size.width * 0.7, b.size.height * 0.6));
            }

            w.setReleasedWhenClosed(false);
            w.setTitle(&NSString::from_str(title));
            let label = NSTextField::labelWithString(
                &NSString::from_str("Temporary Magnetic integration test window"),
                mtm,
            );
            label.setFrame(NSRect::new(NSPoint::new(20., 130.), NSSize::new(360., 24.)));
            w.contentView().unwrap().addSubview(&label);
            w.orderFrontRegardless();
            windows.push(w);
        }
        app.finishLaunching();
        app.run();
        drop(windows);
        Ok(())
    }
}
struct RestoreFocus(Option<objc2::rc::Retained<NSRunningApplication>>);
impl Drop for RestoreFocus {
    fn drop(&mut self) {
        if let Some(app) = &self.0 {
            #[allow(deprecated)]
            app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        }
    }
}
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn smoke() -> Result<(), String> {
    unsafe {
        if !AXIsProcessTrusted() {
            return Err(super::permission_help());
        }
        let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        let _restore_focus = RestoreFocus(NSWorkspace::sharedWorkspace().frontmostApplication());
        let child = ProcessCommand::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--fixture")
            .spawn()
            .map_err(|e| e.to_string())?;
        let fixture = Fixture(child);
        let pid = fixture.0.id() as i32;
        let mut discovery = Discovery::new();
        let mut found = None;
        let discovery_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < discovery_deadline {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.12, false);
            std::thread::sleep(std::time::Duration::from_millis(25));
            let s = discovery.scan(mtm, &[]);
            let ids: Vec<_> = discovery
                .handles
                .values()
                .filter(|h| h.pid == pid && h.visible)
                .map(|h| h.id)
                .collect();
            if ids.len() == 2 {
                found = Some((s, ids));
                break;
            }
        }
        let (snapshot, ids) =
            found.ok_or("Fixture windows were not discoverable through Accessibility/Quartz")?;
        let mut e = Engine::new(8.);
        e.bounds = snapshot.bounds;
        for w in snapshot.windows {
            if ids.contains(&w.id) {
                e.windows.insert(w.id, w);
            }
        }
        e.set_visible(1, &ids, Some(ids[0]));
        let id = ids[0];
        let handle = &discovery.handles[&id];
        let initial = frame(handle.element.0).ok_or("No fixture geometry")?;
        let requested = Rect {
            x: initial.x + 47.,
            y: initial.y + 29.,
            ..initial
        };
        set_position(handle.element.0, requested).map_err(|c| format!("AX move failed: {c}"))?;
        let moved = frame(handle.element.0).ok_or("Cannot read moved geometry")?;
        if !moved.near(requested)
            || (initial.width - moved.width).abs() > 0.1
            || (initial.height - moved.height).abs() > 0.1
        {
            return Err(format!(
                "Position-only move changed dimensions or failed: {initial:?} → {moved:?}"
            ));
        }
        println!("PASS: real AX enumeration, position write, and fixed-size readback");
        #[allow(deprecated)]
        handle
            .app
            .activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
        AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
        let ax_app =
            Owned::new(AXUIElementCreateApplication(pid)).ok_or("Cannot inspect fixture focus")?;
        let mut focused = false;
        for _ in 0..20 {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
            if attr(ax_app.0, "AXFocusedWindow").is_some_and(|w| CFEqual(w.0, handle.element.0))
                && NSWorkspace::sharedWorkspace()
                    .frontmostApplication()
                    .is_some_and(|a| a.processIdentifier() == pid)
            {
                focused = true;
                break;
            }
        }
        if !focused {
            return Err("Native focus/raise did not select the fixture window".into());
        }
        println!("PASS: native application activation and target-window focus/raise");
        // Verify the continuous selector against real AX focus while its HUD stays
        // visible. Only the two child-process fixture windows are candidates.
        let ui = Ui::new(mtm);
        let mut scrub = ScrubSession::new(&e, Axis::Vertical).ok_or("No scrub candidates")?;
        let rectangles: Vec<_> = ids
            .iter()
            .map(|id| frame(discovery.handles[id].element.0))
            .collect();
        for offset in [1, 0, 1, 0] {
            let selected = scrub
                .update(offset)
                .ok_or("Scrub did not change selection")?;
            let target = &discovery.handles[&selected];
            AXUIElementSetAttributeValue(target.element.0, string("AXMain").0, kCFBooleanTrue);
            AXUIElementPerformAction(target.element.0, string("AXRaise").0);
            let mut focused = false;
            for _ in 0..20 {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
                if attr(ax_app.0, "AXFocusedWindow").is_some_and(|w| CFEqual(w.0, target.element.0))
                {
                    focused = true;
                    break;
                }
            }
            if !focused {
                return Err("Live scrub focus did not reach its native target".into());
            }
            e.focus(selected);
            ui.show_navigation(&e, Some(Axis::Vertical));
            if ui.is_key() {
                return Err("Gesture overview stole keyboard focus".into());
            }
        }
        for (id, before) in ids.iter().zip(rectangles) {
            if frame(discovery.handles[id].element.0) != before {
                return Err("Continuous selection changed native geometry".into());
            }
        }
        ui.hide();
        println!(
            "PASS: continuous selection reverses live AX focus with a non-key overview and unchanged geometry"
        );
        e.observed_frame(id, moved, false);
        e.command(Command::ToggleMode);
        let effects = e.command(Command::Snap(Direction::Left));
        let Some(Effect::Snap(_, target)) = effects.first() else {
            return Err("No floating snap effect".into());
        };
        snap_frame(handle.element.0, *target, e.bounds[&e.windows[&id].display])
            .map_err(|c| format!("AX snap failed: {c}"))?;
        let snapped = frame(handle.element.0).ok_or("Cannot read snapped geometry")?;
        if !snapped.near(*target) {
            return Err(format!(
                "App did not accept snap: {snapped:?}, expected {target:?}"
            ));
        }
        println!("PASS: explicit floating-window snap to usable display region");
        let effects = e.command(Command::Snap(Direction::Down));
        let Some(Effect::Snap(_, restore)) = effects.first() else {
            return Err("No restore effect".into());
        };
        set_size(handle.element.0, *restore).map_err(|c| c.to_string())?;
        set_position(handle.element.0, *restore).map_err(|c| c.to_string())?;
        if !frame(handle.element.0)
            .ok_or("No restore geometry")?
            .near(moved)
        {
            return Err("Restore did not recover prior rectangle".into());
        }
        println!("PASS: explicit restore returns the pre-snap rectangle");
        let tap = Owned::new(CGEventTapCreate(
            1,
            0,
            0,
            1 << 12,
            super::event_callback,
            std::ptr::null_mut(),
        ))
        .ok_or("Global keyboard event tap denied")?;
        CGEventTapEnable(tap.0, false);
        println!("PASS: global keyboard event tap can be created");
        ui.show(&e);
        if ui_is_key(&ui) {
            return Err("HUD stole keyboard focus".into());
        }
        let path = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join("tmp/native-smoke-hud.png");
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        ui.snapshot(&path.to_string_lossy())?;
        ui.hide();
        println!("PASS: AppKit HUD renders without becoming key");
        constrained_smoke(mtm)?;
        println!("Native smoke test passed; fixture windows closed automatically.");
        Ok(())
    }
}
fn ui_is_key(ui: &Ui) -> bool {
    ui.is_key()
}

fn constrained_smoke(mtm: MainThreadMarker) -> Result<(), String> {
    let child = ProcessCommand::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("--fixture-minimum-size")
        .spawn()
        .map_err(|e| e.to_string())?;
    let fixture = Fixture(child);
    let pid = fixture.0.id() as i32;
    let mut discovery = Discovery::new();
    let mut found = None;
    for _ in 0..60 {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
        }
        let snapshot = discovery.scan(mtm, &[]);
        if let Some(w) = snapshot
            .windows
            .into_iter()
            .find(|w| discovery.handles[&w.id].pid == pid && snapshot.visible.contains(&w.id))
        {
            let bounds = snapshot.bounds[&w.display];
            found = Some((w.id, bounds));
            break;
        }
    }
    let (id, bounds) = found.ok_or("Minimum-size fixture was not discovered")?;
    let handle = &discovery.handles[&id];
    for region in [
        magnetic::geometry::Region::Right,
        magnetic::geometry::Region::BottomRight,
        magnetic::geometry::Region::TopLeft,
    ] {
        let target = region.rect(bounds);
        snap_frame(handle.element.0, target, bounds)
            .map_err(|c| format!("Constrained fixture snap failed: {c}"))?;
        let actual = frame(handle.element.0).ok_or("Cannot read constrained fixture")?;
        if actual.width <= target.width || !actual.intersection(bounds).near(actual) {
            return Err(format!(
                "Minimum-width snap is not fully visible: {actual:?} in {bounds:?}"
            ));
        }
        if region != magnetic::geometry::Region::TopLeft
            && (actual.right() - bounds.right()).abs() > 1.
        {
            return Err("Minimum-width snap lost right-edge alignment".into());
        }
        if region == magnetic::geometry::Region::BottomRight
            && (actual.bottom() - bounds.bottom()).abs() > 1.
        {
            return Err("Minimum-height snap lost bottom-edge alignment".into());
        }
    }
    println!(
        "PASS: real minimum-width/height windows remain on-screen and preserve right/bottom snap anchors"
    );
    Ok(())
}
