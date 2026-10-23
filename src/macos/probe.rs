//! Opt-in diagnostics. Only fixture windows owned by a child process are manipulated.
use super::{discovery::Discovery, ffi::*, ui::Ui};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::*;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::process::{Child, Command as ProcessCommand};
use whalu::{
    core::{Command, Effect, Engine, Mode, Window},
    geometry::{Direction, Rect},
    gestures::Axis,
    navigation::ScrubSession,
};

/// Read-only timings against the current desktop. No focus or geometry changes.
pub fn performance() -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(super::permission_help());
    }
    let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
    let _app = NSApplication::sharedApplication(mtm);
    let mut discovery = Discovery::new();
    let mut scans = Vec::new();
    let mut visible = 0;
    for _ in 0..5 {
        let start = std::time::Instant::now();
        visible = discovery.scan(mtm, &[]).visible.len();
        scans.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let mut checks = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        if discovery.navigation_focus().is_some() {
            checks.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    scans.sort_by(f64::total_cmp);
    checks.sort_by(f64::total_cmp);
    println!("Visible windows: {visible}");
    println!("Full discovery median (5 scans): {:.2} ms", scans[2]);
    println!("Validated navigation cache hits: {}/20", checks.len());
    if !checks.is_empty() {
        println!(
            "Cached focus/visibility check median: {:.2} ms",
            checks[checks.len() / 2]
        );
    } else {
        println!("Cache unavailable: desktop changed or focus requires full discovery");
    }
    Ok(())
}

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
        (5, "Finder", "Projects", 100.),
        (6, "Safari", "Documentation", 400.),
        (7, "Mail", "Inbox", 500.),
        (8, "Calendar", "Today", 600.),
        (9, "Preview", "Reference", 700.),
        (10, "Music", "Library", 900.),
        (11, "Settings", "Preferences", 1000.),
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
    e.set_visible(1, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], Some(6));
    let ui = Ui::new(mtm);
    ui.verify_wheel_animation(&mut e)?;
    e.focus(6);
    ui.hide();
    ui.show(&e);
    let result = ui.snapshot(path);
    if hold {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
        let mut next = std::time::Instant::now() + std::time::Duration::from_secs(8);
        let sequence = [7, 8, 7, 6, 5, 4, 5, 6, 1, 11, 6];
        let mut index = 0;
        while std::time::Instant::now() < until {
            if std::time::Instant::now() >= next {
                e.focus(sequence[index % sequence.len()]);
                ui.show(&e);
                index += 1;
                next = std::time::Instant::now() + std::time::Duration::from_secs(2);
            }
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
        let titles = [
            "Whalu fixture A",
            "Whalu fixture B",
            "Whalu fixture C",
            "Whalu fixture D",
        ];
        let count = if constrained {
            2
        } else {
            std::env::var("WHALU_FIXTURE_COUNT")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(4)
                .clamp(1, 4)
        };
        let overlap = std::env::var_os("WHALU_FIXTURE_OVERLAP").is_some();
        let same_title = std::env::var_os("WHALU_FIXTURE_SAME_TITLE").is_some();
        for (i, title) in titles.iter().take(count).enumerate() {
            let w = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(
                    NSPoint::new(130. + if overlap { 0. } else { i as f64 * 120. }, 180.),
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
            w.setTitle(&NSString::from_str(if same_title {
                titles[0]
            } else {
                title
            }));
            let label = NSTextField::labelWithString(
                &NSString::from_str("Temporary Whalu integration test window"),
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
            .env("WHALU_FIXTURE_OVERLAP", "1")
            .env("WHALU_FIXTURE_SAME_TITLE", "1")
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
            if ids.len() == 4 {
                found = Some((s, ids));
                break;
            }
        }
        let (snapshot, ids) =
            found.ok_or("Fixture windows were not discoverable through Accessibility/Quartz")?;
        let first_frame =
            frame(discovery.handles[&ids[0]].element.0).ok_or("No overlapping fixture geometry")?;
        if ids.iter().any(|id| {
            !frame(discovery.handles[id].element.0).is_some_and(|r| r.near(first_frame))
                || !discovery.is_onscreen(*id)
        }) {
            return Err(
                "Overlapping fixture windows did not remain independently clickable".into(),
            );
        }
        let refreshed = discovery.scan(mtm, &[]);
        if ids.iter().any(|id| !refreshed.visible.contains(id)) {
            return Err("Refresh lost an overlapping fixture window".into());
        }
        println!(
            "PASS: identical overlapping windows remain independent and clickable after refresh"
        );
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
        // visible. Only child-process fixture windows are candidates.
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
            (1 << 12) | (1 << 22),
            super::event_callback,
            std::ptr::null_mut(),
        ))
        .ok_or("Global keyboard event tap denied")?;
        CGEventTapEnable(tap.0, false);
        println!("PASS: global keyboard and scroll event tap can be created");
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
        super::input_probe::run(e, discovery, ui, tap.clone(), mtm)?;
        cross_app_click_smoke(mtm, tap)?;
        constrained_smoke(mtm)?;
        println!("Native smoke test passed; fixture windows closed automatically.");
        Ok(())
    }
}
fn cross_app_click_smoke(mtm: MainThreadMarker, tap: Owned) -> Result<(), String> {
    let mut fixtures = vec![];
    for _ in 0..4 {
        fixtures.push(Fixture(
            ProcessCommand::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--fixture")
                .env("WHALU_FIXTURE_COUNT", "1")
                .spawn()
                .map_err(|e| e.to_string())?,
        ));
    }
    let pids: Vec<_> = fixtures
        .iter()
        .map(|fixture| fixture.0.id() as i32)
        .collect();
    let mut discovery = Discovery::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
        }
        let snapshot = discovery.scan(mtm, &[]);
        let ids: Vec<_> = pids
            .iter()
            .filter_map(|pid| {
                discovery
                    .handles
                    .values()
                    .find(|handle| handle.pid == *pid && handle.visible)
                    .map(|handle| handle.id)
            })
            .collect();
        if ids.len() != pids.len() {
            continue;
        }
        let mut engine = Engine::new(8.);
        engine.bounds = snapshot.bounds;
        for window in snapshot.windows {
            if ids.contains(&window.id) {
                engine.windows.insert(window.id, window);
            }
        }
        engine.set_visible(1, &ids, Some(ids[0]));
        super::input_probe::run(engine, discovery, Ui::new(mtm), tap, mtm)?;
        println!(
            "PASS: immediate click focus, ordered visual travel and stale-Space isolation across four independent application processes"
        );
        return Ok(());
    }
    Err("Cross-application fixture windows were not discoverable".into())
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
        whalu::geometry::Region::Right,
        whalu::geometry::Region::BottomRight,
        whalu::geometry::Region::TopLeft,
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
        if region != whalu::geometry::Region::TopLeft
            && (actual.right() - bounds.right()).abs() > 1.
        {
            return Err("Minimum-width snap lost right-edge alignment".into());
        }
        if region == whalu::geometry::Region::BottomRight
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
