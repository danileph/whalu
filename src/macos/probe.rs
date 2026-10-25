//! Opt-in diagnostics. Only fixture windows owned by a child process are manipulated.
use super::{discovery::Discovery, ffi::*, ui::Ui};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize,
    NSString,
};
use std::process::{Child, Command as ProcessCommand};
use whalu::{
    core::{Engine, Mode, Window},
    geometry::Rect,
    gestures::Axis,
    navigation::ScrubSession,
};

define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    struct FixtureMenuObserver;
    unsafe impl NSObjectProtocol for FixtureMenuObserver {}
    impl FixtureMenuObserver {
        #[unsafe(method(menuBegan:))]
        fn menu_began(&self, _note: &NSNotification) {
            if let Some(path) = std::env::var_os("WHALU_FIXTURE_MENU_LOG") {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = file.write_all(b"tracking\n");
                }
            }
        }
    }
);

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
        (12, "Photos", "Library", 200.),
        (13, "Reminders", "Today", 300.),
        (14, "Messages", "Conversations", 400.),
        (15, "Activity Monitor", "Processes", 500.),
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
            },
        );
    }
    let ids: Vec<_> = (1..=15).collect();
    e.set_visible(1, &ids, Some(8));
    let ui = Ui::new(mtm);
    ui.verify_wheel_animation(&mut e)?;
    e.focus(8);
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
        let main_menu = NSMenu::new(mtm);
        let app_item = NSMenuItem::new(mtm);
        app_item.setSubmenu(Some(&NSMenu::new(mtm)));
        main_menu.addItem(&app_item);
        let window_item = NSMenuItem::new(mtm);
        window_item.setTitle(&NSString::from_str("Window"));
        let window_menu = NSMenu::new(mtm);
        window_menu.setTitle(&NSString::from_str("Window"));
        let zoom = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Zoom"),
            Some(sel!(zoom:)),
            &NSString::from_str(""),
        );
        window_menu.addItem(&zoom);
        window_item.setSubmenu(Some(&window_menu));
        main_menu.addItem(&window_item);
        app.setMainMenu(Some(&main_menu));
        app.setWindowsMenu(Some(&window_menu));
        let menu_observer: Retained<FixtureMenuObserver> =
            msg_send![FixtureMenuObserver::alloc(mtm), init];
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            &menu_observer,
            sel!(menuBegan:),
            Some(NSMenuDidBeginTrackingNotification),
            None,
        );
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
        // Navigation is invoked on the pointer's monitor. Place this independent
        // fixture cohort there without moving the user's pointer or windows.
        let display = super::discovery::pointer_display(mtm);
        let screen = NSScreen::screens(mtm)
            .iter()
            .find(|screen| Some(super::discovery::display_id(screen)) == display)
            .or_else(|| NSScreen::mainScreen(mtm))
            .ok_or("No fixture screen")?;
        let bounds = screen.visibleFrame();
        for (i, title) in titles.iter().take(count).enumerate() {
            let w = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(
                    NSPoint::new(
                        bounds.origin.x + 130. + if overlap { 0. } else { i as f64 * 120. },
                        bounds.origin.y + 180.,
                    ),
                    NSSize::new(400., 300.),
                ),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            );
            if constrained {
                w.setMinSize(NSSize::new(
                    bounds.size.width * 0.7,
                    bounds.size.height * 0.6,
                ));
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
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        windows[0].makeKeyAndOrderFront(None);
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
        let menu_log =
            std::env::temp_dir().join(format!("whalu-native-menu-{}.log", std::process::id()));
        std::fs::write(&menu_log, "").map_err(|error| error.to_string())?;
        let child = ProcessCommand::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--fixture")
            .env("WHALU_FIXTURE_MENU_LOG", &menu_log)
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
        let captures: std::collections::BTreeSet<_> = ids
            .iter()
            .filter_map(|id| discovery.handles[id].quartz_id)
            .collect();
        if captures.len() != ids.len()
            || ids.iter().any(|id| {
                let handle = &discovery.handles[id];
                handle.quartz_id != window_id(handle.element.0)
            })
        {
            return Err("Identical fixture windows did not receive independent capture IDs".into());
        }
        println!(
            "PASS: identical overlapping windows retain independent capture IDs and click targets after refresh"
        );
        if super::previews::available() {
            let targets = ids
                .iter()
                .map(|id| (*id, discovery.handles[id].quartz_id.unwrap()))
                .collect();
            let (tx, rx) = std::sync::mpsc::channel();
            super::previews::capture_with(targets, move |id, quartz, image| {
                let _ = tx.send((id, quartz, image));
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut captured = std::collections::BTreeSet::new();
            while std::time::Instant::now() < deadline && captured.len() < ids.len() {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
                while let Ok((id, quartz, image)) = rx.try_recv() {
                    let image = image.ok_or("An overlapping fixture thumbnail was empty")?;
                    if discovery.handles[&id].quartz_id != Some(quartz)
                        || objc2_core_graphics::CGImage::width(Some(image.get())) == 0
                        || objc2_core_graphics::CGImage::height(Some(image.get())) == 0
                    {
                        return Err("Fixture thumbnail did not match its window capture ID".into());
                    }
                    captured.insert(id);
                }
            }
            if captured.len() != ids.len() {
                return Err(
                    "Timed out capturing independent overlapping fixture thumbnails".into(),
                );
            }
            println!("PASS: real screenshot content for every identical overlapping window");
        } else {
            println!("SKIP: thumbnail capture requires Screen Recording permission");
        }
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
        let before_tiling = frame(handle.element.0).ok_or("Missing native tiling fixture frame")?;
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(123, false))?;
        settle_native_tiling();
        let tiled = frame(handle.element.0).ok_or("Missing native tiled frame")?;
        if tiled.near(before_tiling) {
            return Err("Native left-half action did not change the fixture".into());
        }
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(15, false))?;
        settle_native_tiling();
        if !frame(handle.element.0).is_some_and(|actual| actual.near(before_tiling)) {
            return Err("Native restore did not recover the fixture rectangle".into());
        }
        println!("PASS: native Window-menu left-half tiling and macOS previous-size restoration");
        let center_original = frame(handle.element.0).ok_or("No center fixture frame")?;
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(8, false))?;
        settle_native_tiling();
        let centered = frame(handle.element.0).ok_or("No centered fixture frame")?;
        let center_bounds = e.bounds[&e.windows[&id].display];
        if (centered.width - center_original.width).abs() > 2.
            || (centered.height - center_original.height).abs() > 2.
            || (centered.center().0 - center_bounds.center().0).abs() > 2.
            || (centered.center().1 - center_bounds.center().1).abs() > 2.
        {
            return Err(format!(
                "Center did not preserve size and center the normal fixture: {center_original:?} -> {centered:?}"
            ));
        }
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(3, false))?;
        settle_native_tiling();
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(8, false))?;
        settle_native_tiling();
        let centered_from_fill = frame(handle.element.0).ok_or("No centered restored fixture")?;
        if (centered_from_fill.width - center_original.width).abs() > 2.
            || (centered_from_fill.height - center_original.height).abs() > 2.
        {
            return Err(format!(
                "Center retained the filled rectangle: {center_original:?} -> {centered_from_fill:?}"
            ));
        }
        println!(
            "PASS: Center preserves a normal window size and restores pre-tiling size after Fill"
        );
        // Repeated arrangements used to leave AppKit in menu tracking and stall
        // subsequent keyboard navigation. Exercise every command and then reuse
        // cached commands, while the fixture records any tracking notification.
        let mut max_dispatch = std::time::Duration::ZERO;
        for _ in 0..3 {
            for (key, arrange) in [
                (123, false),
                (124, false),
                (126, false),
                (125, false),
                (3, false),
                (8, false),
                (123, true),
                (124, true),
                (126, true),
                (125, true),
                (15, false),
            ] {
                let start = std::time::Instant::now();
                super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(key, arrange))?;
                max_dispatch = max_dispatch.max(start.elapsed());
                // Yield for native handling without waiting for a full animation.
                std::thread::sleep(std::time::Duration::from_millis(35));
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false);
            }
        }
        settle_native_tiling();
        AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
        AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
        let worker_before = frame(handle.element.0).ok_or("No worker fixture frame")?;
        let submitted = std::time::Instant::now();
        super::tiling::submit(
            pid,
            handle.element.0,
            whalu::input::NativeShortcut::tiling(124, false),
        )?;
        let submission = submitted.elapsed();
        settle_native_tiling();
        if frame(handle.element.0).is_none_or(|actual| actual.near(worker_before)) {
            return Err("Background native tiling did not reach the focused fixture".into());
        }
        super::tiling::perform(pid, whalu::input::NativeShortcut::tiling(15, false))?;
        settle_native_tiling();
        println!(
            "PASS: background native action executes without blocking submission ({:.1} ms)",
            submission.as_secs_f64() * 1000.
        );
        if !std::fs::read(&menu_log)
            .map_err(|error| error.to_string())?
            .is_empty()
        {
            return Err("Native tiling opened an AppKit menu tracking loop".into());
        }
        std::fs::remove_file(&menu_log).map_err(|error| error.to_string())?;
        println!(
            "PASS: 33 repeated native tiling/arrangement/restore actions without menu tracking; maximum dispatch {:.1} ms",
            max_dispatch.as_secs_f64() * 1000.
        );

        let quartz = handle.quartz_id.ok_or("Fixture has no Quartz identity")?;
        let before_transfer = frame(handle.element.0).ok_or("No fixture frame")?;
        let source_space = super::spaces::window_space(quartz)?.ok_or("No unique fixture Space")?;
        let direction = if super::spaces::move_adjacent(handle.element.0, quartz, true)? {
            Some(true)
        } else if super::spaces::move_adjacent(handle.element.0, quartz, false)? {
            Some(false)
        } else {
            None
        };
        if let Some(direction) = direction {
            // Follow back through the actual background command queue, with
            // exact-window focus confirmation after the desktop transition.
            AXUIElementSetAttributeValue(ax_app.0, string("AXFocusedWindow").0, handle.element.0);
            AXUIElementSetAttributeValue(handle.element.0, string("AXFocused").0, kCFBooleanTrue);
            AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
            AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
            AXUIElementSetAttributeValue(ax_app.0, string("AXFrontmost").0, kCFBooleanTrue);
            super::tiling::submit_space(pid, handle.element.0, quartz, !direction)?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(4);
            loop {
                if super::spaces::window_space(quartz)? == Some(source_space)
                    && discovery.is_onscreen(id)
                    && attr(ax_app.0, "AXFocusedWindow")
                        .is_some_and(|focused| CFEqual(focused.0, handle.element.0))
                    && NSWorkspace::sharedWorkspace()
                        .frontmostApplication()
                        .is_some_and(|app| app.processIdentifier() == pid)
                {
                    break;
                }
                if std::time::Instant::now() >= until {
                    return Err("Queued Space follow did not return the visible, focused fixture to its original desktop".into());
                }
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, false);
            if !frame(handle.element.0).is_some_and(|actual| actual.near(before_transfer)) {
                return Err("Native Space transfer changed fixture geometry".into());
            }
            println!(
                "PASS: real fixture moves and follows the adjacent native Space, then returns with unchanged geometry"
            );
        } else {
            println!("SKIP: Space transfer requires two regular desktops on this display");
        }
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
        // Bundled child processes share one application identity. Close this
        // cohort before the next probe so its windows cannot join that stack.
        drop(fixture);
        cross_app_click_smoke(mtm, tap)?;
        println!("Native smoke test passed; fixture windows closed automatically.");
        Ok(())
    }
}
/// Run input regressions with fresh windows, independently of native tiling and
/// Space transitions that can leave the earlier smoke cohort without AX focus.
pub fn input_smoke() -> Result<(), String> {
    unsafe {
        if !AXIsProcessTrusted() {
            return Err(super::permission_help());
        }
        let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        let _restore_focus = RestoreFocus(NSWorkspace::sharedWorkspace().frontmostApplication());
        let tap = Owned::new(CGEventTapCreate(
            1,
            0,
            0,
            (1 << 12) | (1 << 22),
            super::event_callback,
            std::ptr::null_mut(),
        ))
        .ok_or("Global keyboard and scroll event tap denied")?;
        CGEventTapEnable(tap.0, false);
        cross_app_click_smoke(mtm, tap)
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

fn settle_native_tiling() {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while std::time::Instant::now() < until {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.03, false);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Exercise only native Space transfers, without preceding tiling animations.
pub fn space_smoke() -> Result<(), String> {
    unsafe {
        let mtm = MainThreadMarker::new().ok_or("Requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        if !AXIsProcessTrusted() {
            return Err(super::permission_help());
        }
        let _restore = RestoreFocus(NSWorkspace::sharedWorkspace().frontmostApplication());
        let fixture = Fixture(
            ProcessCommand::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--fixture")
                .spawn()
                .map_err(|e| e.to_string())?,
        );
        let pid = fixture.0.id() as i32;
        let mut discovery = Discovery::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let id = loop {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
            let snapshot = discovery.scan(mtm, &[]);
            if let Some(id) = snapshot
                .focused
                .filter(|id| discovery.handles[id].pid == pid)
            {
                break id;
            }
            if std::time::Instant::now() >= deadline {
                return Err("No focused Space fixture".into());
            }
        };
        let handle = &discovery.handles[&id];
        let quartz = handle.quartz_id.ok_or("No fixture Quartz ID")?;
        let original = frame(handle.element.0).ok_or("No fixture frame")?;
        let source = super::spaces::window_space(quartz)?.ok_or("No source desktop")?;
        let siblings: Vec<_> = discovery
            .handles
            .values()
            .filter(|h| h.pid == pid && h.id != id)
            .filter_map(|h| h.quartz_id)
            .collect();
        if siblings.is_empty() {
            return Err("Missing source-only witness windows".into());
        }
        let direction = if super::spaces::move_adjacent(handle.element.0, quartz, true)? {
            true
        } else if super::spaces::move_adjacent(handle.element.0, quartz, false)? {
            false
        } else {
            return Err("Native Space test needs two neighboring regular desktops".into());
        };
        let target = super::spaces::window_space(quartz)?.ok_or("No destination desktop")?;
        let app = Owned::new(AXUIElementCreateApplication(pid)).ok_or("No fixture app")?;
        AXUIElementSetAttributeValue(app.0, string("AXFocusedWindow").0, handle.element.0);
        AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
        AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
        AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.15, false);
        let snapshot = discovery.scan(mtm, &[]);
        let display = snapshot
            .windows
            .iter()
            .find(|w| w.id == id)
            .ok_or("Missing transferred fixture")?
            .display;
        let ids: Vec<_> = discovery
            .handles
            .values()
            .filter(|h| h.pid == pid && h.visible)
            .map(|h| h.id)
            .collect();
        let mut engine = Engine::new(8.);
        engine.bounds = snapshot.bounds;
        for window in snapshot.windows {
            if discovery
                .handles
                .get(&window.id)
                .is_some_and(|h| h.pid == pid)
            {
                engine.windows.insert(window.id, window);
            }
        }
        engine.set_display_visible(display, 1, &ids, Some(id));
        super::input_probe::space_moves(
            engine,
            discovery,
            super::input_probe::SpaceScenario {
                id,
                original,
                source_space: source,
                target_space: target,
                direction,
                siblings,
            },
            mtm,
        )?;
        Ok(())
    }
}
