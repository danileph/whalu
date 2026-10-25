//! Exercise production scroll lifecycle against only the native smoke fixtures.
//! State persistence is disabled; no synthetic input is posted to the desktop.
use super::*;

pub(super) fn run(
    engine: Engine,
    discovery: Discovery,
    ui: ui::Ui,
    tap: Owned,
    mtm: MainThreadMarker,
) -> Result<(), String> {
    let ids = engine
        .context()
        .ok_or("Missing input probe context")?
        .order
        .clone();
    let fixture_pids: BTreeSet<_> = ids.iter().map(|id| discovery.handles[id].pid).collect();
    let fixture_app_ids: BTreeSet<_> = ids
        .iter()
        .map(|id| engine.windows[id].app_id.clone())
        .collect();
    let (mut runtime, tx) = fixture_runtime(engine, discovery, ui, tap, mtm)?;
    // The preceding non-key HUD snapshot and Space transition can leave
    // AppKit between activation callbacks (the fixture shares our bundle ID).
    // Establish this keyboard scenario's focus explicitly, then verify every
    // command changes the real focused window as before.
    let first = &runtime.discovery.handles[&ids[0]];
    let app = unsafe { Owned::new(AXUIElementCreateApplication(first.pid)) }
        .ok_or("No keyboard fixture app")?;
    unsafe {
        AXUIElementSetAttributeValue(app.0, string("AXFocusedWindow").0, first.element.0);
        AXUIElementSetAttributeValue(first.element.0, string("AXMain").0, kCFBooleanTrue);
        AXUIElementPerformAction(first.element.0, string("AXRaise").0);
        AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while !attr(app.0, "AXFocusedWindow").is_some_and(|w| unsafe { CFEqual(w.0, first.element.0) })
    {
        if Instant::now() >= deadline {
            return Err("Keyboard fixture did not establish focus".into());
        }
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
        }
    }
    runtime.reconcile(false);
    // Use restored bindings through the actual Runtime command path, after
    // repeated native tiling in the smoke test, and confirm AX focus each time.
    runtime.held = true;
    runtime.navigation_display = runtime.engine.display();
    for key in [125, 126, 38, 40, 124, 123, 37, 4] {
        let command = whalu::input::route(
            &runtime.config.bindings,
            key,
            Modifiers {
                control: true,
                option: true,
                ..Default::default()
            },
        )
        .ok_or("Missing Control-Option keyboard navigation binding")?;
        runtime.prepare_command(command);
        let expected = match runtime.engine.command(command).first() {
            Some(Effect::Focus(id)) => *id,
            None => runtime
                .engine
                .focused()
                .ok_or("No keyboard fixture focus")?,
        };
        tx.send(Event::Command(command))
            .map_err(|error| error.to_string())?;
        runtime.drain();
        if runtime.engine.focused() != Some(expected) {
            return Err("Keyboard command failed after native tiling".into());
        }
        let target = &runtime.discovery.handles[&expected];
        let app = unsafe { Owned::new(AXUIElementCreateApplication(target.pid)) }
            .ok_or("No keyboard fixture app")?;
        let deadline = Instant::now() + Duration::from_millis(800);
        while !attr(app.0, "AXFocusedWindow")
            .is_some_and(|focused| unsafe { CFEqual(focused.0, target.element.0) })
        {
            if Instant::now() >= deadline {
                return Err("Keyboard navigation did not change actual AX focus".into());
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    runtime.held = false;
    runtime.navigation_display = None;
    runtime.ui.hide();
    println!(
        "PASS: Control-Option arrow and Vim keyboard commands retain real window/app focus after repeated native tiling"
    );
    command_switching(&mut runtime, &tx, &ids)?;
    let prepare = |runtime: &mut Runtime, source| -> Result<(), String> {
        runtime.clear_scroll();
        runtime.apply(vec![Effect::Focus(ids[0])]);
        runtime.gesture = Some(ActiveScroll {
            source,
            session: ScrubSession::new(&runtime.engine, Axis::Vertical)
                .ok_or("No fixture scroll session")?,
            motion: KineticScroll::default(),
            wheel: WheelScroll::default(),
            position: 0.,
            last_time: 0.,
            last_input: Instant::now(),
            released: false,
            changed: false,
            sustained: false,
        });
        Ok(())
    };
    // The single-app pass covers synthetic rapid gesture bursts. The multi-app
    // pass below specifically verifies click focus and visual travel, allowing
    // normal application activation to settle between distinct input actions.
    if fixture_pids.len() == 1 {
        prepare(&mut runtime, ScrollSource::Trackpad(1))?;
        runtime.gesture.as_mut().unwrap().motion.sample(0., 0.);
        runtime.gesture.as_mut().unwrap().motion.sample(0.5, 0.08);
        // A short gesture below a whole selection interval still enters the next window.
        runtime.gesture_event(1, GestureEvent::End { cancelled: false }, 0.09);
        if runtime.engine.focused() != Some(ids[1])
            || !runtime.ui.is_visible()
            || runtime.ui.is_key()
            || runtime.gesture.is_some()
        {
            return Err("Short-swipe selection or non-key feedback failed".into());
        }
        let epoch = runtime.ui.visibility_epoch();
        for (index, direction) in [-1, 1, -1, 1, -1, 1].into_iter().enumerate() {
            tx.send(Event::ContactBegan).map_err(|e| e.to_string())?;
            runtime.drain();
            if !runtime.ui.is_visible() || runtime.ui.visibility_epoch() != epoch {
                return Err("Rapid swipe contact hid the overview between strokes".into());
            }
            if index == 0 {
                runtime.deadline = Some(Instant::now() + Duration::from_millis(70));
            }
            runtime.gesture_event(1, GestureEvent::Begin(Axis::Vertical), 0.);
            if runtime.ui.visibility_epoch() != epoch {
                return Err("Rapid swipe restart briefly hid the overview".into());
            }
            runtime.gesture_event(1, GestureEvent::Update(direction), 0.1);
            runtime.gesture_event(1, GestureEvent::End { cancelled: false }, 0.12);
            let expected = if direction < 0 { ids[0] } else { ids[1] };
            if runtime.engine.focused() != Some(expected)
                || !runtime.ui.is_visible()
                || runtime.ui.visibility_epoch() != epoch
            {
                return Err(
                    "Rapid swipe failed to change exactly one fixture without flicker".into(),
                );
            }
        }
        let deadline = runtime.linger_until.ok_or("Feedback delay missing")?;
        if deadline.duration_since(Instant::now()) < Duration::from_millis(550) {
            return Err("Overview feedback delay is too short".into());
        }
        runtime.advance_scroll(Instant::now() + Duration::from_millis(250));
        if !runtime.ui.is_visible() {
            return Err("Overview hid before feedback completed".into());
        }
        runtime.advance_scroll(deadline + Duration::from_millis(1));
        if runtime.ui.is_visible() {
            return Err("Overview did not dismiss after its delay".into());
        }

        prepare(&mut runtime, ScrollSource::Trackpad(1))?;
        for (time, position) in [(0., 0.), (0.04, 0.4), (0.08, 1.), (0.12, 1.6)] {
            runtime
                .gesture
                .as_mut()
                .unwrap()
                .motion
                .sample(position, time);
        }
        runtime.gesture.as_mut().unwrap().sustained = true;
        runtime.release_scroll(0.13);
        if !runtime
            .gesture
            .as_ref()
            .is_some_and(|active| active.motion.coasting())
        {
            return Err("A released flick did not coast".into());
        }
        let mut time = runtime.motion_tick;
        for _ in 0..100 {
            time += Duration::from_millis(16);
            runtime.advance_scroll(time);
            if runtime.gesture.is_none() {
                break;
            }
        }
        if runtime.engine.focused() != ids.last().copied()
            || runtime.gesture.is_some()
            || runtime.motion_due.is_some()
            || !runtime.ui.is_visible()
        {
            return Err("Coast failed to select, stop, or retain feedback".into());
        }

        prepare(&mut runtime, ScrollSource::Wheel)?;
        runtime.wheel_event(1., 0.);
        runtime.advance_scroll(Instant::now() + Duration::from_millis(112));
        if runtime.engine.focused() != Some(ids[1])
            || runtime.gesture.is_some()
            || runtime.linger_until.is_none()
        {
            return Err("Modifier-wheel burst failed to finish with feedback".into());
        }
        tx.send(Event::ContactBegan).map_err(|e| e.to_string())?;
        runtime.drain();
        if runtime.linger_until.is_none() || !runtime.ui.is_visible() {
            return Err("Fresh contact did not preserve feedback for the next swipe".into());
        }
        // Click a real fixture thumbnail while a coast owns the overview. Routing
        // is exercised without posting synthetic input to other applications.
    }
    thumbnail_reordering(&mut runtime, &tx, &ids)?;
    prepare(&mut runtime, ScrollSource::Trackpad(1))?;
    runtime.render();
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.4, false);
    }
    let point = runtime
        .ui
        .thumbnail_point(ids[1], objc2_foundation::NSPoint::new(64., 41.))?;
    let ui::ClickRoute::Select {
        context,
        window,
        epoch,
    } = thumbnail_click(&mut runtime, &tx, point)?
    else {
        return Err("Fixture thumbnail click missed".into());
    };
    tx.send(Event::SelectThumbnail {
        context,
        window,
        epoch,
    })
    .map_err(|e| e.to_string())?;
    runtime.drain();
    let handle = &runtime.discovery.handles[&ids[1]];
    let app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
        .ok_or("Missing fixture application")?;
    let native_focused = attr(app.0, "AXFocusedWindow")
        .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) });
    if runtime.engine.focused() != Some(ids[1])
        || !native_focused
        || runtime.gesture.is_some()
        || runtime.motion_due.is_some()
        || !runtime.ui.is_visible()
        || runtime.ui.is_key()
        || runtime.ui.visibility_epoch() != epoch
        || runtime.linger_until.is_none()
        || runtime.ui.clicks.route(2, point) != ui::ClickRoute::Pass
    {
        return Err("Thumbnail click failed native focus, selection, coast interruption or continuous non-key feedback".into());
    }
    // Stale context/card messages cannot navigate an unrelated Space/window.
    for (context, window) in [(context.wrapping_add(1), ids[0]), (context, u64::MAX)] {
        tx.send(Event::SelectThumbnail {
            context,
            window,
            epoch,
        })
        .map_err(|e| e.to_string())?;
        runtime.drain();
        if runtime.engine.focused() != Some(ids[1]) {
            return Err("Stale thumbnail click changed selection".into());
        }
    }
    println!(
        "PASS: thumbnail click selects and focuses its native fixture, cancels scrolling, preserves overview feedback and rejects stale targets"
    );
    // Exercise a distant click in both directions using four real windows.
    // Each timer turn must raise exactly one adjacent fixture, even if overdue.
    for reverse in [false, true] {
        runtime.clear_scroll();
        let path: Vec<_> = if reverse {
            ids.iter().rev().copied().collect()
        } else {
            ids.clone()
        };
        // Seed the opposite native order, so raising only the destination
        // cannot accidentally satisfy the compositor-order assertion below.
        for window in path.iter().rev() {
            runtime.apply(vec![Effect::Focus(*window)]);
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.04, false);
            }
        }
        let original_frames: Vec<_> = path
            .iter()
            .map(|id| frame(runtime.discovery.handles[id].element.0))
            .collect();
        runtime.linger_until = Some(Instant::now() + Duration::from_millis(650));
        runtime.render();
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.4, false);
        }
        // A post-Space census may be unfocused or stale even though native AX
        // focus has now settled. The first click must use the fresh local focus.
        runtime
            .discovery
            .scan(runtime.mtm, &runtime.config.ignored_apps);
        runtime.pending_focus = None;
        if reverse {
            runtime.engine.focus(*path.last().unwrap());
        } else {
            let context = runtime.engine.current;
            runtime.engine.contexts.get_mut(&context).unwrap().focused = None;
        }
        let point = runtime.ui.thumbnail_point(
            *path.last().unwrap(),
            objc2_foundation::NSPoint::new(64., 41.),
        )?;
        let ui::ClickRoute::Select {
            context,
            window,
            epoch,
        } = thumbnail_click(&mut runtime, &tx, point)?
        else {
            return Err("Distant fixture thumbnail click missed".into());
        };
        tx.send(Event::SelectThumbnail {
            context,
            window,
            epoch,
        })
        .map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.ui.clicks.route(2, point);
        for (index, expected) in path.iter().enumerate().skip(1) {
            if index > 1 {
                let due = runtime
                    .click_travel
                    .as_ref()
                    .ok_or("Click travel stopped early")?
                    .due;
                let previous = runtime.engine.focused();
                runtime.advance_click(due - Duration::from_millis(1));
                if runtime.engine.focused() != previous {
                    return Err("Click skipped its visible step interval".into());
                }
                runtime.advance_click(due + Duration::from_secs(1));
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.04, false);
            }
            let handle = &runtime.discovery.handles[expected];
            let target_app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
                .ok_or("Missing target app")?;
            let actual = attr(target_app.0, "AXFocusedWindow")
                .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
                && NSWorkspace::sharedWorkspace()
                    .frontmostApplication()
                    .is_some_and(|app| app.processIdentifier() == handle.pid);
            if runtime.engine.focused() != Some(*expected)
                || runtime.ui.selected_thumbnail() != Some(*expected)
                || !actual
                || !runtime.ui.is_visible()
                || runtime.ui.is_key()
                || runtime.ui.visibility_epoch() != epoch
            {
                return Err(format!("Click travel skipped fixture {index} in {path:?}"));
            }
        }
        if runtime.click_travel.is_some()
            || runtime.linger_until.is_none()
            || runtime.engine.context().unwrap().order != ids
        {
            return Err("Click travel failed to stop with feedback and stable stack order".into());
        }
        let expected_order: Vec<_> = path
            .iter()
            .rev()
            .map(|id| {
                runtime.discovery.handles[id]
                    .quartz_id
                    .ok_or("Missing fixture Quartz ID")
            })
            .collect::<Result<_, _>>()?;
        let deadline = Instant::now() + Duration::from_millis(800);
        loop {
            let list = unsafe { Owned::new(CGWindowListCopyWindowInfo(17, 0)) }
                .ok_or("Missing compositor window list")?;
            let actual_order: Vec<_> = unsafe { array(list.0) }
                .into_iter()
                .map(|window| unsafe { number(dict(window, "kCGWindowNumber")) as u32 })
                .filter(|window| expected_order.contains(window))
                .collect();
            if actual_order == expected_order {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "Click did not restore native stack order: {actual_order:?}, expected {expected_order:?}"
                ));
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
            }
        }
        let final_frames: Vec<_> = path
            .iter()
            .map(|id| frame(runtime.discovery.handles[id].element.0))
            .collect();
        if final_frames != original_frames {
            return Err("Click traversal changed native window geometry".into());
        }
    }
    let context = runtime.engine.current;
    runtime.select_thumbnail(context, ids[3]);
    if runtime.engine.focused() != Some(ids[1])
        || runtime.ui.selected_thumbnail() != Some(ids[1])
        || runtime.click_travel.is_none()
    {
        return Err("Distant click did not raise its first adjacent window".into());
    }
    runtime.select_thumbnail(context, ids[0]);
    runtime.advance_click(Instant::now() + Duration::from_secs(1));
    if runtime.engine.focused() != Some(ids[0]) || runtime.click_travel.is_some() {
        return Err("Second click did not replace the old destination".into());
    }
    runtime.select_thumbnail(context, ids[3]);
    runtime.contact_began();
    if runtime.engine.focused() != Some(ids[1]) || runtime.click_travel.is_none() {
        return Err("Ordinary finger contact interrupted click travel".into());
    }
    if fixture_pids.len() > 1 {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.12, false);
        }
    }
    runtime.gesture_event(1, GestureEvent::Begin(Axis::Vertical), 0.);
    if runtime.click_travel.is_some() || runtime.gesture.is_none() {
        return Err("Recognized swipe did not catch click travel".into());
    }
    runtime.clear_scroll();
    runtime.apply(vec![Effect::Focus(ids[0])]);
    runtime.linger_until = Some(Instant::now() + Duration::from_millis(650));
    runtime.render();
    runtime.select_thumbnail(context, ids[3]);
    if fixture_pids.len() > 1 {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.12, false);
        }
    }
    tx.send(Event::Command(Command::FocusPrevious))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.click_travel.is_some() || runtime.engine.focused() != Some(ids[0]) {
        return Err(format!(
            "Keyboard input did not catch click travel: focused {:?}, expected {}, order {:?}, frozen {}, context {}",
            runtime.engine.focused(),
            ids[0],
            runtime.engine.context().map(|ctx| &ctx.order),
            runtime.engine.frozen,
            runtime.engine.current
        ));
    }
    println!(
        "PASS: first click refreshes missing/stale recorded focus, native traversal and compositor order in both directions, unchanged geometry, feedback, retargeting and interruption"
    );
    // A queued click from before a Space transition must remain invalid even
    // when the same cohort/context identifier is reused for another overview.
    runtime.linger_until = Some(Instant::now() + Duration::from_millis(650));
    runtime.render();
    let old_epoch = runtime.ui.visibility_epoch();
    let focused = runtime.engine.focused();
    runtime.ui.reset_overview();
    if runtime.ui.selected_thumbnail().is_some()
        || runtime.ui.clicks.route(1, point) != ui::ClickRoute::Pass
    {
        return Err("Space invalidation retained old cards or hit targets".into());
    }
    runtime.render();
    tx.send(Event::SelectThumbnail {
        context: runtime.engine.current,
        window: ids[0],
        epoch: old_epoch,
    })
    .map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.engine.focused() != focused || !runtime.ui.is_visible() {
        return Err("Old overview click affected a freshly shown context".into());
    }
    println!(
        "PASS: Space reset discards cached cards/hit targets and rejects queued stale clicks even with a reused context ID"
    );
    // A fourth contact arrives before NSWorkspace reports the Space change.
    // Its cancelled three-finger End must not restore the original window.
    for (source, released) in [
        (ScrollSource::Trackpad(1), false),
        (ScrollSource::Trackpad(1), true),
        (ScrollSource::Wheel, true),
    ] {
        prepare(&mut runtime, source)?;
        if fixture_pids.len() > 1 {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.12, false) };
        }
        runtime.select_scroll(1);
        if fixture_pids.len() > 1 {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.12, false) };
        }
        runtime.gesture.as_mut().unwrap().released = released;
        runtime.motion_due = Some(Instant::now());
        let selected = runtime.engine.focused();
        let before = frame(runtime.discovery.handles[&ids[0]].element.0)
            .ok_or("Missing fixture frame before native gesture")?;
        runtime.pending_commands.push(Command::FocusNext);
        tx.send(Event::NativeGesture).map_err(|e| e.to_string())?;
        tx.send(Event::Gesture(
            1,
            GestureEvent::End { cancelled: true },
            0.2,
        ))
        .map_err(|e| e.to_string())?;
        tx.send(Event::Gesture(
            1,
            GestureEvent::Begin(Axis::Horizontal),
            0.21,
        ))
        .map_err(|e| e.to_string())?;
        tx.send(Event::Gesture(1, GestureEvent::Update(1), 0.22))
            .map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.apply(vec![Effect::Focus(ids[0])]);
        if runtime.engine.focused() != selected
            || runtime.gesture.is_some()
            || runtime.motion_due.is_some()
            || runtime.pending_focus.is_some()
            || !runtime.pending_commands.is_empty()
            || runtime.space_until.is_none()
            || !runtime.engine.frozen
            || runtime.ui.is_visible()
            || frame(runtime.discovery.handles[&ids[0]].element.0) != Some(before)
        {
            return Err("Four-finger handoff restored focus or modified a fixture window".into());
        }
        // Input queued before the eventual workspace notification must not be
        // replayed into its destination (or another rapid Space transition).
        tx.send(Event::Command(Command::FocusNext))
            .map_err(|e| e.to_string())?;
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        runtime.drain();
        if !runtime.pending_commands.is_empty() {
            return Err("Space change retained navigation from before the notification".into());
        }
        runtime.space_until = Some(Instant::now());
        runtime.drain();
        let handle = &runtime.discovery.handles[&ids[1]];
        let app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
            .ok_or("Missing native gesture fixture app")?;
        if runtime.engine.focused() != selected
            || !attr(app.0, "AXFocusedWindow")
                .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
        {
            return Err(
                "Native gesture cancellation or settlement changed real window focus".into(),
            );
        }
    }
    // Even before a workspace notification, cached visible=true must not be
    // sufficient to raise a window that fresh Quartz metadata no longer shows.
    let selected = runtime.engine.focused();
    let old_capture = runtime.discovery.handles[&ids[0]].quartz_id;
    runtime
        .discovery
        .handles
        .get_mut(&ids[0])
        .unwrap()
        .quartz_id = Some(u32::MAX);
    runtime.apply(vec![Effect::Focus(ids[0])]);
    runtime
        .discovery
        .handles
        .get_mut(&ids[0])
        .unwrap()
        .quartz_id = old_capture;
    if runtime.engine.focused() != selected {
        return Err("Cached visibility allowed focus on a window absent from Quartz".into());
    }
    println!(
        "PASS: four-finger handoff abandons active navigation and momentum before cancellation, blocks stale focus/moves, and settles without restoring the old window"
    );
    prepare(&mut runtime, ScrollSource::Trackpad(1))?;
    tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.gesture.is_some()
        || runtime.click_travel.is_some()
        || runtime.linger_until.is_some()
        || runtime.ui.is_visible()
    {
        return Err("Space change did not abandon navigation".into());
    }
    // A transition may briefly enumerate no windows. Recovery must be driven
    // by bounded timers, without requiring another gesture/key to wake it.
    let ignored = runtime.config.ignored_apps.clone();
    runtime
        .config
        .ignored_apps
        .extend(fixture_app_ids.iter().cloned());
    runtime.space_until = Some(Instant::now());
    runtime.held = true;
    runtime.drain();
    if runtime.ui.is_visible() || runtime.deadline.is_none() || runtime.space_retries != 7 {
        return Err("Empty Space snapshot did not schedule a bounded retry".into());
    }
    runtime.config.ignored_apps = ignored;
    runtime.deadline = Some(Instant::now());
    runtime.drain();
    if !runtime.ui.is_visible()
        || runtime.space_retries != 0
        || runtime
            .engine
            .context()
            .is_none_or(|ctx| ctx.order.is_empty())
    {
        return Err("Overview did not recover automatically after an empty Space snapshot".into());
    }
    println!(
        "PASS: empty Space snapshot schedules a bounded retry and overview recovers without new user input"
    );
    display_isolation(&mut runtime, &ids)?;
    if let Some(timer) = runtime.timer.take() {
        unsafe {
            CFRunLoopTimerInvalidate(timer.0);
        }
    }
    println!(
        "PASS: real fixture short-swipe focus, kinetic selection, wheel burst, 650ms non-key feedback, dismissal, rapid-swipe visibility continuity and Space interruption; no recovery writes"
    );
    Ok(())
}

fn thumbnail_reordering(
    runtime: &mut Runtime,
    tx: &Sender<Event>,
    ids: &[u64],
) -> Result<(), String> {
    use objc2_foundation::NSPoint;
    let original = runtime.engine.context().unwrap().order.clone();
    let frames: Vec<_> = ids
        .iter()
        .map(|id| frame(runtime.discovery.handles[id].element.0))
        .collect();
    for held in [true, false] {
        for (source, target) in [(1, 2), (3, 0)] {
            runtime.clear_scroll();
            runtime.apply(vec![Effect::Focus(ids[1])]);
            runtime.held = held;
            runtime.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
            runtime.render();
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.4, false);
            }
            let focus = runtime.engine.focused();
            let from = runtime
                .ui
                .thumbnail_point(ids[source], NSPoint::new(64., 41.))?;
            let to = runtime
                .ui
                .thumbnail_point(ids[target], NSPoint::new(64., 41.))?;
            if route_pointer(runtime, tx, 1, from)? != ui::ClickRoute::Consume
                || route_pointer(runtime, tx, 6, to)? != ui::ClickRoute::Consume
            {
                return Err("Drag did not capture a fixture thumbnail".into());
            }
            runtime.held = false;
            runtime.advance_scroll(Instant::now() + Duration::from_secs(2));
            runtime.render();
            if !runtime.ui.is_visible()
                || runtime.ui.is_key()
                || runtime.engine.context().unwrap().order != original
                || runtime.engine.focused() != focus
            {
                return Err(
                    "Provisional drag changed focus/order or lost the overview after release"
                        .into(),
                );
            }
            route_pointer(runtime, tx, 2, to)?;
            let mut expected = original.clone();
            let moved = expected.remove(source);
            expected.insert(target, moved);
            if runtime.engine.context().unwrap().order != expected
                || runtime.engine.focused() != focus
                || runtime.ui.pointer_active()
                || runtime.linger_until.is_none()
            {
                return Err(
                    "Thumbnail drop did not commit its insertion while preserving focus".into(),
                );
            }
            // The regular census must retain the new order.
            runtime.reconcile(false);
            if runtime.engine.context().unwrap().order != expected {
                return Err("Discovery discarded the dropped order".into());
            }
            runtime.engine.reorder(ids[source], source);
        }
    }
    for reset in [false, true] {
        runtime.clear_scroll();
        runtime.held = true;
        runtime.render();
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.4, false);
        }
        let from = runtime.ui.thumbnail_point(ids[1], NSPoint::new(64., 41.))?;
        route_pointer(runtime, tx, 1, from)?;
        let outside = NSPoint::new(-10000., -10000.);
        route_pointer(runtime, tx, 6, outside)?;
        if reset {
            runtime.ui.reset_overview();
        }
        if route_pointer(runtime, tx, 2, outside)? != ui::ClickRoute::Consume
            || runtime.engine.context().unwrap().order != original
            || runtime.ui.pointer_active()
        {
            return Err("Cancelled drag changed order or leaked mouse-up".into());
        }
        runtime.render();
    }
    if ids
        .iter()
        .map(|id| frame(runtime.discovery.handles[id].element.0))
        .collect::<Vec<_>>()
        != frames
    {
        return Err("Thumbnail reorder moved a native window".into());
    }
    println!(
        "PASS: mouse drag inserts in both directions during held and transient overviews, keeps focus/geometry, survives modifier release and census, and cancels outside/stale drops"
    );
    runtime.held = false;
    runtime.clear_scroll();
    runtime.render();
    Ok(())
}

fn route_pointer(
    runtime: &mut Runtime,
    tx: &Sender<Event>,
    kind: u32,
    point: objc2_foundation::NSPoint,
) -> Result<ui::ClickRoute, String> {
    let route = runtime.ui.clicks.route(kind, point);
    if let ui::ClickRoute::Pointer {
        context,
        window,
        epoch,
        kind,
        point,
    } = route
    {
        tx.send(Event::ThumbnailPointer {
            context,
            window,
            epoch,
            kind,
            point,
        })
        .map_err(|e| e.to_string())?;
        runtime.drain();
        Ok(ui::ClickRoute::Consume)
    } else {
        Ok(route)
    }
}
fn thumbnail_click(
    runtime: &mut Runtime,
    tx: &Sender<Event>,
    point: objc2_foundation::NSPoint,
) -> Result<ui::ClickRoute, String> {
    if route_pointer(runtime, tx, 1, point)? != ui::ClickRoute::Consume {
        return Err("Thumbnail press missed".into());
    }
    route_pointer(runtime, tx, 2, point)
}

fn command_switching(runtime: &mut Runtime, tx: &Sender<Event>, ids: &[u64]) -> Result<(), String> {
    let command = Modifiers {
        command: true,
        ..Default::default()
    };
    for key in [48, 50] {
        runtime.clear_scroll();
        runtime.apply(vec![Effect::Focus(ids[1])]);
        let mut router = ShortcutRouter::default();
        let KeyRoute::Consume(Some(action)) =
            router.key(&runtime.config.bindings, key, 1 << 20, true)
        else {
            return Err("Missing Command window-switch binding".into());
        };
        tx.send(Event::OverviewHeld(
            router.overview_held(command, runtime.config.overlay_modifiers),
        ))
        .map_err(|e| e.to_string())?;
        tx.send(Event::WindowSwitch(action))
            .map_err(|e| e.to_string())?;
        runtime.drain();
        let epoch = runtime.ui.visibility_epoch();
        runtime.advance_scroll(Instant::now() + Duration::from_secs(2));
        runtime.render();
        if !runtime.ui.is_visible() || runtime.ui.is_key() {
            return Err("Held Command shortcut lost its non-key overview".into());
        }
        let before = runtime.engine.focused();
        let delta = if key == 48 { -1. } else { 1. };
        tx.send(Event::Wheel(delta, 0.))
            .map_err(|e| e.to_string())?;
        runtime.drain();
        if runtime.engine.focused() == before || runtime.ui.visibility_epoch() != epoch {
            return Err("Wheel did not navigate within the held Command overview".into());
        }
        runtime.clear_scroll();
        // Cover key-first release for Tab and modifier-first release for grave.
        if key == 48 {
            router.key(&[], key, 1 << 20, false);
        }
        let modifiers = if key == 48 {
            command
        } else {
            Modifiers::default()
        };
        tx.send(Event::OverviewHeld(
            router.overview_held(modifiers, runtime.config.overlay_modifiers),
        ))
        .map_err(|e| e.to_string())?;
        runtime.drain();
        if !runtime.ui.is_visible() || runtime.ui.visibility_epoch() != epoch {
            return Err("First shortcut release hid the held overview".into());
        }
        router.key(&[], key, 0, false);
        tx.send(Event::OverviewHeld(router.overview_held(
            Modifiers::default(),
            runtime.config.overlay_modifiers,
        )))
        .map_err(|e| e.to_string())?;
        runtime.drain();
        if !runtime.ui.is_visible() || runtime.ui.visibility_epoch() != epoch {
            return Err("Quick Command release hid its overview feedback".into());
        }
        let mut deadline = runtime.linger_until.ok_or("No Command release feedback")?;
        if deadline.duration_since(Instant::now()) < Duration::from_millis(550) {
            return Err("Command release feedback was shorter than swipe feedback".into());
        }
        // Release both keys between each quick tap. Alternating directions must
        // navigate immediately, retain the same cards, and restart dismissal.
        for next_key in [50, 48, 50, 48, 50, 48] {
            let mut router = ShortcutRouter::default();
            let KeyRoute::Consume(Some(action)) =
                router.key(&runtime.config.bindings, next_key, 1 << 20, true)
            else {
                return Err("Missing rapid Command switch binding".into());
            };
            let before = runtime.engine.focused();
            tx.send(Event::OverviewHeld(true))
                .map_err(|e| e.to_string())?;
            tx.send(Event::WindowSwitch(action))
                .map_err(|e| e.to_string())?;
            tx.send(Event::OverviewHeld(false))
                .map_err(|e| e.to_string())?;
            runtime.drain();
            let next_deadline = runtime.linger_until.ok_or("Rapid tap lost feedback")?;
            if runtime.engine.focused() == before
                || !runtime.ui.is_visible()
                || runtime.ui.visibility_epoch() != epoch
                || next_deadline <= deadline
            {
                return Err(
                    "Rapid Command taps lost focus, overview continuity, or dismissal renewal"
                        .into(),
                );
            }
            deadline = next_deadline;
        }
        runtime.advance_scroll(deadline - Duration::from_millis(1));
        if !runtime.ui.is_visible() {
            return Err("Command overview hid before its final feedback expired".into());
        }
        runtime.advance_scroll(deadline + Duration::from_millis(1));
        if runtime.ui.is_visible() || runtime.linger_until.is_some() {
            return Err("Command overview did not dismiss after the final tap's delay".into());
        }
    }
    println!(
        "PASS: Command-Tab/grave retain non-key overview through holds, wheel navigation, either release order and rapid separate taps, then dismiss after 650ms"
    );
    Ok(())
}

fn display_isolation(runtime: &mut Runtime, ids: &[u64]) -> Result<(), String> {
    let displays: Vec<_> = runtime.engine.bounds.keys().copied().take(2).collect();
    if displays.len() < 2 {
        println!("SKIP: native monitor isolation requires two connected displays");
        return Ok(());
    }
    // This is a new external-focus scenario, independent of the preceding
    // click probe's bounded focus confirmation and feedback timers.
    runtime.pending_focus = None;
    runtime.clear_scroll();
    // Move only the four disposable child windows: two per real monitor.
    for (index, id) in ids.iter().enumerate() {
        let bounds = runtime.engine.bounds[&displays[index / 2]];
        let element = runtime.discovery.handles[id].element.0;
        let before = frame(element).ok_or("Missing monitor fixture geometry")?;
        set_position(
            element,
            whalu::geometry::Rect {
                x: bounds.x + 100. + (index % 2) as f64 * 140.,
                y: bounds.y + 100.,
                ..before
            },
        )
        .map_err(|code| format!("Could not move monitor fixture: AX {code}"))?;
    }
    let mut previous = None;
    for side in [0, 1, 0] {
        let selected = ids[side * 2];
        let neighbor = ids[side * 2 + 1];
        let foreign = ids[(1 - side) * 2];
        let handle = &runtime.discovery.handles[&selected];
        let app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
            .ok_or("Missing monitor fixture app")?;
        unsafe {
            AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
            AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
            #[allow(deprecated)]
            handle
                .app
                .activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        }
        let mut confirmed = false;
        for _ in 0..20 {
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false) };
            if attr(app.0, "AXFocusedWindow")
                .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
                && NSWorkspace::sharedWorkspace()
                    .frontmostApplication()
                    .is_some_and(|app| app.processIdentifier() == handle.pid)
            {
                confirmed = true;
                break;
            }
        }
        if !confirmed {
            return Err("External focus did not reach the monitor fixture".into());
        }
        // Exercise the fast navigation path after focus changes displays while
        // the global Quartz cohort stays unchanged. Supply the monitor where
        // this overview was invoked; leave the real desktop pointer untouched.
        runtime.navigation_display = Some(displays[side]);
        runtime.held = true;
        runtime.prepare_command(Command::FocusNext);
        runtime.overlay_deadline = None;
        runtime.render();
        let expected = BTreeSet::from([selected, neighbor]);
        if runtime.engine.display() != Some(displays[side])
            || runtime.engine.focused() != Some(selected)
            || runtime
                .engine
                .context()
                .unwrap()
                .order
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != expected
            || runtime.ui.selected_thumbnail() != Some(selected)
            || runtime
                .ui
                .thumbnail_point(foreign, objc2_foundation::NSPoint::new(64., 41.))
                .is_ok()
            || runtime
                .discovery
                .is_onscreen_on_display(foreign, Some((displays[side], &runtime.engine.bounds)))
        {
            return Err(format!(
                "Monitor isolation failed: expected display {} and windows {:?}, display {:?}, focused {:?} (expected {}), order {:?}, selected thumbnail {:?}, foreign thumbnail {}, foreign still on display {}, foreign frame {:?}",
                displays[side],
                expected,
                runtime.engine.display(),
                runtime.engine.focused(),
                selected,
                runtime.engine.context().map(|context| &context.order),
                runtime.ui.selected_thumbnail(),
                runtime
                    .ui
                    .thumbnail_point(foreign, objc2_foundation::NSPoint::new(64., 41.))
                    .is_ok(),
                runtime.discovery.is_onscreen_on_display(
                    foreign,
                    Some((displays[side], &runtime.engine.bounds))
                ),
                frame(runtime.discovery.handles[&foreign].element.0)
            ));
        }
        if let Some((context, window)) = previous {
            runtime.select_thumbnail(context, window);
            if runtime.engine.focused() != Some(selected) {
                return Err("A stale monitor thumbnail changed focus".into());
            }
        }
        if runtime.engine.command(Command::FocusNext) != [Effect::Focus(neighbor)] {
            return Err("Keyboard navigation escaped the active monitor".into());
        }
        previous = Some((runtime.engine.current, selected));
    }
    pointer_overview(runtime, ids, &displays)?;
    runtime.ui.hide();
    println!(
        "PASS: real fixtures on two monitors retain separate overview/navigation stacks, refresh cached external focus, and reject foreign/stale thumbnail targets"
    );
    Ok(())
}

fn pointer_overview(runtime: &mut Runtime, ids: &[u64], displays: &[u32]) -> Result<(), String> {
    let focused = ids[0];
    let handle = &runtime.discovery.handles[&focused];
    let focused_pid = handle.pid;
    // Discovery replaces AX proxies during the refreshes below. Retain the
    // independent reference used to verify that native focus stays unchanged.
    let element = handle.element.clone();
    let app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
        .ok_or("Missing pointer overview fixture app")?;
    let is_focused = || {
        attr(app.0, "AXFocusedWindow").is_some_and(|window| unsafe { CFEqual(window.0, element.0) })
    };
    if !is_focused() {
        return Err("Pointer overview probe requires focus on the first monitor".into());
    }
    runtime.pending_focus = None;
    runtime.clear_scroll();
    // Emulate the display sampled from the pointer when opening: monitor 2,
    // while the actual AX focus remains on monitor 1. Do not post mouse input.
    runtime.navigation_display = Some(displays[1]);
    runtime.held = true;
    runtime.overlay_deadline = None;
    runtime.reconcile(false);
    for _ in 0..2 {
        runtime.reconcile(false);
        if runtime.engine.display() != Some(displays[1])
            || runtime.engine.focused().is_some()
            || runtime
                .engine
                .context()
                .unwrap()
                .order
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != BTreeSet::from([ids[2], ids[3]])
            || !runtime.ui.is_visible()
            || runtime.ui.is_key()
            || !is_focused()
        {
            return Err(
                "Pointer overview returned to the focused monitor or stole native focus".into(),
            );
        }
    }
    runtime.prepare_command(Command::ToggleMode);
    if runtime.engine.display() != Some(displays[0]) || runtime.engine.focused() != Some(focused) {
        return Err("A focused-window command followed the pointer overview's monitor".into());
    }
    runtime.navigation_display = Some(displays[1]);
    runtime.reconcile(false);
    // Cached keyboard preparation must stay on the requested monitor even
    // though navigation_focus() still reports a window on the other monitor.
    runtime.prepare_command(Command::FocusNext);
    let effects = runtime.engine.command(Command::FocusNext);
    if !matches!(effects.as_slice(), [Effect::Focus(id)] if ids[2..].contains(id)) {
        return Err("Pointer overview keyboard navigation returned to the focused monitor".into());
    }
    runtime.held = false;
    runtime.render();
    runtime.drain();
    if runtime.navigation_display.is_some() || runtime.ui.is_visible() {
        return Err("Closing the overview retained its old monitor selection".into());
    }
    for source in [ScrollSource::Trackpad(1), ScrollSource::Wheel] {
        runtime.begin_scroll_on_display(source, Axis::Vertical, Some(displays[1]));
        let target = runtime
            .gesture
            .as_mut()
            .and_then(|active| active.session.update(1));
        if runtime.engine.display() != Some(displays[1])
            || !target.is_some_and(|id| ids[2..].contains(&id))
            || !runtime.ui.is_visible()
            || !is_focused()
        {
            return Err("Pointer gesture/wheel overview required focus on its monitor".into());
        }
        runtime.clear_scroll();
        runtime.render();
        runtime.drain();
    }
    // Exercise a real absence of local native focus: another monitor owns AX
    // focus, as can happen when entering a Space before a window is activated.
    let local_order = runtime.engine.context().unwrap().order.clone();
    let anchor = local_order[0];
    let destination = local_order[1];
    for remembered in [None, Some(anchor)] {
        runtime.navigation_display = Some(displays[1]);
        runtime.held = true;
        runtime.pending_focus = None;
        runtime.reconcile(false);
        let context = runtime.engine.current;
        let local = runtime.engine.contexts.get_mut(&context).unwrap();
        local.focused = None;
        local.last_focused = remembered;
        runtime.render();
        runtime.select_thumbnail(context, destination);
        if runtime.engine.focused() != Some(anchor) || runtime.click_travel.is_none() {
            return Err(format!(
                "First unfocused click skipped anchor {anchor} for {destination}: focused {:?}, native {:?}, travel {}, order {:?}, visible {}, frozen {}, space {}, remembered {remembered:?}",
                runtime.engine.focused(),
                runtime.discovery.navigation_focus(),
                runtime.click_travel.is_some(),
                runtime.engine.context().map(|ctx| &ctx.order),
                runtime.ui.is_visible(),
                runtime.engine.frozen,
                runtime.space_until.is_some(),
            ));
        }
        let due = runtime.click_travel.as_ref().unwrap().due;
        runtime.advance_click(due);
        if runtime.engine.focused() != Some(destination) || runtime.click_travel.is_some() {
            return Err("First unfocused click did not traverse to its destination".into());
        }
        runtime.clear_scroll();
        runtime.navigation_display = Some(displays[0]);
        runtime.reconcile(false);
        runtime.apply(vec![Effect::Focus(focused)]);
        unsafe {
            AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while !is_focused()
            || NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_none_or(|app| app.processIdentifier() != focused_pid)
        {
            if Instant::now() >= deadline {
                return Err("Could not restore the foreign-monitor fixture focus".into());
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
            }
        }
    }
    println!(
        "PASS: first click with absent local native focus enters the remembered/first anchor and traverses to its destination without borrowing another monitor's focus"
    );
    runtime.navigation_display = Some(displays[0]);
    runtime.held = true;
    runtime.reconcile(false);
    if runtime.engine.display() != Some(displays[0]) || !is_focused() {
        return Err("Reopening the overview did not choose the new monitor".into());
    }
    println!(
        "PASS: pointer monitor overview opens without prior focus, survives refreshes and keyboard/gesture/wheel preparation, preserves native focus, and retargets after closing"
    );
    Ok(())
}

fn fixture_runtime(
    engine: Engine,
    discovery: Discovery,
    ui: ui::Ui,
    tap: Owned,
    mtm: MainThreadMarker,
) -> Result<(Runtime, Sender<Event>), String> {
    let (tx, rx) = mpsc::channel();
    let ids = engine
        .context()
        .ok_or("Missing input probe context")?
        .order
        .clone();
    let fixture_pids: BTreeSet<_> = ids.iter().map(|id| discovery.handles[id].pid).collect();
    let fixture_app_ids: BTreeSet<_> = ids
        .iter()
        .map(|id| engine.windows[id].app_id.clone())
        .collect();
    let ignored_apps = NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|app| !fixture_pids.contains(&app.processIdentifier()))
        .map(|app| {
            app.bundleIdentifier()
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("pid:{}", app.processIdentifier()))
        })
        .filter(|id| !fixture_app_ids.contains(id))
        .collect();
    let overview_space = engine
        .display()
        .and_then(|display| super::spaces::active_space(display).map(|space| (display, space)));
    let runtime = Runtime {
        mtm,
        rx,
        config: Config {
            magnetic: false,
            ignored_apps,
            ..Config::default()
        },
        config_path: std::env::temp_dir().join("whalu-input-probe.toml"),
        state_writer: None,
        engine,
        discovery,
        ui,
        observers: BTreeMap::new(),
        tap: tap.clone(),
        gestures: None,
        gesture: None,
        click_travel: None,
        pending_focus: None,
        motion_due: None,
        linger_until: None,
        motion_tick: Instant::now(),
        _tap_source: tap,
        timer: None,
        deadline: None,
        overlay_deadline: None,
        held: false,
        switch_feedback: false,
        mouse: false,
        dragging: false,
        manual_until: None,
        switched_space: false,
        pending_commands: vec![],
        space_until: None,
        space_retries: 0,
        space_transfer_display: None,
        space_transfer_target: None,
        space_transfer_window: None,
        space_overview: false,
        overview_space,
        overview_refresh_pending: Cell::new(false),
        modal: false,
        recovery: SavedState::default(),
        restored: BTreeSet::new(),
        context_tracker: DisplayContexts::default(),
        navigation_display: None,
        _lock: std::fs::File::open("/dev/null").map_err(|e| e.to_string())?,
    };
    Ok((runtime, tx))
}

pub(super) struct SpaceScenario {
    pub id: u64,
    pub original: whalu::geometry::Rect,
    pub source_space: u64,
    pub target_space: u64,
    pub direction: bool,
    pub siblings: Vec<u32>,
}
pub(super) fn space_moves(
    engine: Engine,
    discovery: Discovery,
    scenario: SpaceScenario,
    mtm: MainThreadMarker,
) -> Result<(), String> {
    let SpaceScenario {
        id,
        original,
        source_space,
        target_space,
        direction,
        siblings,
    } = scenario;
    unsafe {
        let tap = Owned::new(CGEventTapCreate(
            1,
            0,
            0,
            1 << 12,
            super::event_callback,
            null_mut(),
        ))
        .ok_or("Cannot create fixture tap")?;
        CGEventTapEnable(tap.0, false);
        let (mut runtime, tx) = fixture_runtime(engine, discovery, ui::Ui::new(mtm), tap, mtm)?;
        let mut context = SourceContext {
            version: 0,
            info: null_mut(),
            retain: None,
            release: None,
            description: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: None,
        };
        let source = Owned::new(CFRunLoopSourceCreate(null(), 0, &mut context))
            .ok_or("Cannot create probe dispatch")?;
        struct DispatchRestore(Option<(Sender<Event>, usize)>);
        impl Drop for DispatchRestore {
            fn drop(&mut self) {
                *DISPATCH.lock().unwrap() = self.0.take();
            }
        }
        let _restore = DispatchRestore(
            DISPATCH
                .lock()
                .unwrap()
                .replace((tx.clone(), source.0 as usize)),
        );
        let pointer = || {
            Owned::new(CGEventCreate(null()))
                .map(|event| CGEventGetLocation(event.0))
                .ok_or("No pointer state")
        };
        let cursor = pointer()?;
        let modifiers = Modifiers {
            control: true,
            option: true,
            command: true,
            shift: false,
        };
        tx.send(Event::OverviewHeld(
            modifiers.overlay(runtime.config.overlay_modifiers),
        ))
        .map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.overlay_deadline = None;
        runtime.render();
        let check_previews = super::previews::available();
        runtime.ui.preload_previews(&runtime.discovery);
        if check_previews {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !runtime.ui.cached_preview(id) {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false);
                runtime.drain();
                if Instant::now() >= deadline {
                    return Err("Visible transfer preview did not warm".into());
                }
            }
        }
        let animations_before = super::spaces::animated_transfers();
        let mut cursor_changed = None;
        for step in 0..5 {
            let returning = step % 2 == 0;
            let expected = if returning {
                source_space
            } else {
                target_space
            };
            let quartz = runtime.discovery.handles[&id]
                .quartz_id
                .ok_or("No transfer identity")?;
            tx.send(Event::Command(Command::MoveSpace(if returning {
                !direction
            } else {
                direction
            })))
            .map_err(|e| e.to_string())?;
            runtime.drain();
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            runtime.drain();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if !runtime.held || !runtime.ui.is_visible() || runtime.ui.is_key() {
                    return Err("Held-modifier overview disappeared during Space transfer".into());
                }
                let position = pointer()?;
                if (position.x - cursor.x).abs() > 0.01 || (position.y - cursor.y).abs() > 0.01 {
                    cursor_changed = Some(format!(
                        "Pointer moved during probe: ({}, {}) -> ({}, {}); let the in-flight native gesture finish before reporting this interrupted check",
                        cursor.x, cursor.y, position.x, position.y
                    ));
                }
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false);
                runtime.drain();
                if runtime.space_until.is_none()
                    && super::spaces::window_space(quartz)? == Some(expected)
                {
                    let order = &runtime.engine.context().ok_or("No destination list")?.order;
                    if !order.contains(&id)
                        || order.len() != if returning { siblings.len() + 1 } else { 1 }
                    {
                        return Err(format!(
                            "Overview did not replace its window list on destination: {order:?}"
                        ));
                    }
                    if runtime.engine.focused() != Some(id)
                        || !super::spaces::is_onscreen(quartz)
                        || siblings
                            .iter()
                            .any(|id| super::spaces::is_onscreen(*id) != returning)
                    {
                        return Err("Space transfer lost focus or retained source windows".into());
                    }
                    if !frame(runtime.discovery.handles[&id].element.0)
                        .is_some_and(|actual| actual.near(original))
                    {
                        return Err("Space transfer changed geometry".into());
                    }
                    if check_previews {
                        let current = runtime.engine.context().unwrap().order.clone();
                        if step == 0 {
                            let deadline = Instant::now() + Duration::from_secs(3);
                            while current.iter().any(|id| !runtime.ui.cached_preview(*id)) {
                                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false);
                                runtime.drain();
                                if Instant::now() >= deadline {
                                    return Err("First destination previews did not load".into());
                                }
                            }
                        } else if current.iter().any(|id| !runtime.ui.cached_preview(*id)) {
                            return Err(
                                "Returning to a Space discarded an already loaded preview".into()
                            );
                        }
                    }
                    if let Some(error) = cursor_changed {
                        return Err(error);
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Held-modifier Space transfer timed out".into());
                }
            }
        }
        if check_previews {
            println!(
                "PASS: destination previews survive all subsequent Space round trips without a blank recapture"
            );
        }
        let animations = super::spaces::animated_transfers() - animations_before;
        println!("Native Dock animations observed: {animations}/5");
        if animations != 5 {
            return Err("Native Dock slide animation was not observed for every Space move".into());
        }
        // Late workspace notifications must refresh without closing a held HUD.
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        runtime.drain();
        if !runtime.ui.is_visible() {
            return Err("Late Space notification hid held overview".into());
        }
        runtime.space_until = Some(Instant::now());
        runtime.drain();
        let refocus = |runtime: &Runtime| -> Result<(), String> {
            let h = &runtime.discovery.handles[&id];
            let app = Owned::new(AXUIElementCreateApplication(h.pid))
                .ok_or("No manual-Space fixture app")?;
            AXUIElementSetAttributeValue(app.0, string("AXFocusedWindow").0, h.element.0);
            AXUIElementSetAttributeValue(h.element.0, string("AXMain").0, kCFBooleanTrue);
            AXUIElementPerformAction(h.element.0, string("AXRaise").0);
            AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
            Ok(())
        };
        // Change the native Space without a Runtime MoveSpace command or a
        // workspace notification. A late preview render must never flash the
        // previous context, even before the notification reaches the app.
        let h = &runtime.discovery.handles[&id];
        let quartz = h.quartz_id.ok_or("Missing manual-Space identity")?;
        if !super::spaces::move_adjacent(h.element.0, quartz, direction)? {
            return Err("No manual-Space fixture destination".into());
        }
        refocus(&runtime)?;
        runtime.render();
        if runtime.ui.is_visible() {
            return Err(
                "Late render showed the previous Space stack before its workspace notification"
                    .into(),
            );
        }
        runtime.drain();
        if !runtime.ui.is_visible()
            || runtime.engine.context().is_none_or(|c| c.order != [id])
            || super::spaces::active_space(runtime.engine.display().ok_or("No overview display")?)
                != Some(target_space)
        {
            return Err("Overview did not refresh immediately to the real native Space".into());
        }
        // Opening while the native navigation pause is still active must read
        // the destination, instead of using the previous stack for 850 ms.
        tx.send(Event::OverviewHeld(false))
            .map_err(|e| e.to_string())?;
        tx.send(Event::NativeGesture).map_err(|e| e.to_string())?;
        runtime.drain();
        let h = &runtime.discovery.handles[&id];
        super::spaces::move_adjacent(h.element.0, quartz, !direction)?;
        refocus(&runtime)?;
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        tx.send(Event::OverviewHeld(
            modifiers.overlay(runtime.config.overlay_modifiers),
        ))
        .map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.overlay_deadline = Some(Instant::now());
        runtime.drain();
        if runtime.space_until.is_none()
            || !runtime.engine.frozen
            || !runtime.ui.is_visible()
            || runtime
                .engine
                .context()
                .is_none_or(|c| c.order.len() != siblings.len() + 1)
            || super::spaces::active_space(
                runtime
                    .engine
                    .display()
                    .ok_or("No returned overview display")?,
            ) != Some(source_space)
        {
            return Err("Opening during a native transition reused the previous stack or waited for the navigation pause".into());
        }
        println!(
            "PASS: external Space changes reject stale renders before notification; opening during the navigation pause shows the destination stack immediately"
        );
        tx.send(Event::OverviewHeld(false))
            .map_err(|e| e.to_string())?;
        runtime.drain();
        if runtime.ui.is_visible() {
            return Err("Overview remained visible after modifier release".into());
        }
        println!(
            "PASS: five Runtime Space moves with modifiers continuously held, unchanged cursor, visible non-key overview, refreshed destination lists, preserved focus/geometry, and source-window isolation"
        );
        Ok(())
    }
}
