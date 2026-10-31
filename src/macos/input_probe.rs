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
    right_edge_hover(&mut runtime, &tx)?;
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
        let target = *path.last().unwrap();
        let immediate = independent_applications(&runtime, &path);
        if immediate {
            if runtime.engine.focused() != Some(target)
                || runtime.ui.selected_thumbnail() != Some(target)
                || runtime
                    .click_travel
                    .as_ref()
                    .is_none_or(|travel| travel.background.is_none())
            {
                return Err("Eligible click did not select its destination immediately".into());
            }
            finish_click(&mut runtime, Some(target))?;
        } else {
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
                wait_native_focus(&runtime, *expected)?;
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
                    return Err(format!(
                        "Click travel skipped fixture {index} in {path:?}: engine {:?}, card {:?}, native {actual}, visible {}, key {}, epoch {} expected {epoch}",
                        runtime.engine.focused(),
                        runtime.ui.selected_thumbnail(),
                        runtime.ui.is_visible(),
                        runtime.ui.is_key(),
                        runtime.ui.visibility_epoch()
                    ));
                }
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
    background_interruptions(&mut runtime, &ids)?;
    runtime.clear_scroll();
    runtime.apply(vec![Effect::Focus(ids[0])]);
    runtime.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
    runtime.render();
    let context = runtime.engine.current;
    let immediate = independent_applications(&runtime, &ids);
    let first = if immediate { ids[3] } else { ids[1] };
    runtime.select_thumbnail(context, ids[3]);
    if runtime.engine.focused() != Some(first)
        || runtime.ui.selected_thumbnail() != Some(first)
        || runtime.click_travel.is_none()
    {
        return Err("Distant click did not raise its first adjacent window".into());
    }
    runtime.select_thumbnail(context, ids[0]);
    finish_click(&mut runtime, None)?;
    if runtime.engine.focused() != Some(ids[0]) || runtime.click_travel.is_some() {
        return Err("Second click did not replace the old destination".into());
    }
    runtime.select_thumbnail(context, ids[3]);
    runtime.contact_began();
    if runtime.engine.focused() != Some(first) || runtime.click_travel.is_none() {
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
    let expected = if immediate { ids[2] } else { ids[0] };
    if runtime.click_travel.is_some() || runtime.engine.focused() != Some(expected) {
        return Err(format!(
            "Keyboard input did not catch click travel: focused {:?}, expected {}, order {:?}, frozen {}, context {}",
            runtime.engine.focused(),
            expected,
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
    contact_overview(&mut runtime, &tx)?;
    rapid_swipes(&mut runtime, &tx, &ids)?;
    post_space_swipe(&mut runtime, &tx)?;
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
            AXUIElementSetAttributeValue(app.0, string("AXFocusedWindow").0, handle.element.0);
            AXUIElementSetAttributeValue(handle.element.0, string("AXFocused").0, kCFBooleanTrue);
            AXUIElementSetAttributeValue(handle.element.0, string("AXMain").0, kCFBooleanTrue);
            AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
            #[allow(deprecated)]
            handle
                .app
                .activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
            AXUIElementSetAttributeValue(app.0, string("AXFrontmost").0, kCFBooleanTrue);
        }
        let mut confirmed = false;
        for _ in 0..20 {
            // Activation can restore this app's previous key window, especially
            // when its documents span monitors. Once the correct process is
            // active, establish the exact external fixture window again.
            if NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|app| app.processIdentifier() == handle.pid)
            {
                unsafe {
                    AXUIElementSetAttributeValue(
                        app.0,
                        string("AXFocusedWindow").0,
                        handle.element.0,
                    );
                    AXUIElementSetAttributeValue(
                        handle.element.0,
                        string("AXMain").0,
                        kCFBooleanTrue,
                    );
                    AXUIElementPerformAction(handle.element.0, string("AXRaise").0);
                }
            }
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
        let immediate = independent_applications(runtime, &[anchor, destination]);
        let first = if immediate { destination } else { anchor };
        if runtime.engine.focused() != Some(first) || runtime.click_travel.is_none() {
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
        finish_click(runtime, None)?;
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

fn independent_applications(runtime: &Runtime, ids: &[u64]) -> bool {
    let mut processes = BTreeSet::new();
    let mut bundles = BTreeSet::new();
    ids.iter().all(|id| {
        let handle = &runtime.discovery.handles[id];
        processes.insert(handle.pid)
            && handle
                .app
                .bundleIdentifier()
                .is_none_or(|bundle| bundles.insert(bundle.to_string()))
    })
}
fn wait_native_focus(runtime: &Runtime, target: u64) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(800);
    while !runtime.native_focus_is(target) {
        if Instant::now() >= deadline {
            return Err(format!("Native focus did not settle on fixture {target}"));
        }
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false) };
    }
    Ok(())
}
fn finish_click(runtime: &mut Runtime, immediate: Option<u64>) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut confirmed = false;
    while runtime.click_travel.is_some() {
        if Instant::now() >= deadline {
            return Err("Click/background ordering did not finish".into());
        }
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.008, false) };
        runtime.advance_click(Instant::now());
        if let Some(target) = immediate {
            let native = runtime.native_focus_is(target);
            if runtime.engine.focused() != Some(target)
                || runtime.ui.selected_thumbnail() != Some(target)
                || (confirmed && !native)
                || runtime
                    .click_travel
                    .as_ref()
                    .is_some_and(|travel| travel.background.is_none())
            {
                return Err(format!(
                    "Background repair lost target {target}: engine {:?}, card {:?}, native {native}, confirmed {confirmed}, background {:?}",
                    runtime.engine.focused(),
                    runtime.ui.selected_thumbnail(),
                    runtime.click_travel.as_ref().map(|travel| travel
                        .background
                        .as_ref()
                        .map(|repair| (repair.raised, repair.waiting)))
                ));
            }
            confirmed |= native;
        }
    }
    Ok(())
}
fn background_interruptions(runtime: &mut Runtime, ids: &[u64]) -> Result<(), String> {
    if !independent_applications(runtime, ids) {
        return Ok(());
    }
    for scenario in 0..5 {
        runtime.clear_scroll();
        for id in ids.iter().rev() {
            runtime.apply(vec![Effect::Focus(*id)]);
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.04, false) };
        }
        runtime.linger_until = Some(Instant::now() + OVERVIEW_FEEDBACK);
        runtime.render();
        runtime.select_thumbnail(runtime.engine.current, ids[3]);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !runtime.native_focus_is(ids[3]) {
            if Instant::now() >= deadline {
                return Err("No immediate fixture destination focus".into());
            }
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false) };
        }
        runtime
            .click_travel
            .as_mut()
            .ok_or("No background repair")?
            .background
            .as_mut()
            .ok_or("No fast path")?
            .confirmed = true;
        match scenario {
            0 => {
                // Native external focus changes before a delayed model census.
                runtime.apply(vec![Effect::Focus(ids[0])]);
                let deadline = Instant::now() + Duration::from_secs(1);
                while !runtime.native_focus_is(ids[0]) {
                    if Instant::now() >= deadline {
                        return Err("No external fixture focus".into());
                    }
                    unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, false) };
                }
                runtime.engine.focus(ids[3]);
                runtime.advance_click(Instant::now() + Duration::from_millis(10));
                if runtime.click_travel.is_some() || !runtime.native_focus_is(ids[0]) {
                    return Err("Background repair reclaimed external focus".into());
                }
            }
            1 => {
                let original = runtime.discovery.handles[&ids[1]].quartz_id;
                runtime
                    .discovery
                    .handles
                    .get_mut(&ids[1])
                    .unwrap()
                    .quartz_id = Some(u32::MAX);
                runtime.advance_click(Instant::now() + Duration::from_millis(10));
                runtime
                    .discovery
                    .handles
                    .get_mut(&ids[1])
                    .unwrap()
                    .quartz_id = original;
                if runtime.click_travel.is_some() || !runtime.native_focus_is(ids[3]) {
                    return Err("Background repair used a stale native identity".into());
                }
            }
            2 => {
                // Simulate a delayed accepted raise. Keep waiting while its
                // compositor deadline is live, then stop without replaying any
                // foreground windows if that order never arrives.
                let repair = runtime
                    .click_travel
                    .as_mut()
                    .unwrap()
                    .background
                    .as_mut()
                    .unwrap();
                repair.waiting = true;
                repair.until = Instant::now() + Duration::from_millis(250);
                runtime.advance_click(Instant::now() + Duration::from_millis(10));
                if runtime.click_travel.is_none()
                    || runtime.engine.focused() != Some(ids[3])
                    || !runtime.native_focus_is(ids[3])
                {
                    return Err(
                        "Delayed background ordering did not retain destination focus".into(),
                    );
                }
                runtime
                    .click_travel
                    .as_mut()
                    .unwrap()
                    .background
                    .as_mut()
                    .unwrap()
                    .until = Instant::now() - Duration::from_millis(1);
                runtime.advance_click(Instant::now() + Duration::from_millis(10));
                if runtime.click_travel.is_some()
                    || runtime.engine.focused() != Some(ids[3])
                    || runtime.ui.selected_thumbnail() != Some(ids[3])
                    || !runtime.native_focus_is(ids[3])
                    || runtime.deadline.is_none()
                {
                    return Err("Failed background ordering replayed intermediate focus".into());
                }
                // A later timer turn must not resume the consumed focus path.
                runtime.advance_click(Instant::now() + Duration::from_secs(1));
                if runtime.engine.focused() != Some(ids[3]) || !runtime.native_focus_is(ids[3]) {
                    return Err("Finished background repair resumed foreground traversal".into());
                }
            }
            _ => {
                if scenario == 3 {
                    runtime.ui.reset_overview();
                } else {
                    runtime
                        .click_travel
                        .as_mut()
                        .unwrap()
                        .background
                        .as_mut()
                        .unwrap()
                        .space = u64::MAX;
                }
                runtime.advance_click(Instant::now() + Duration::from_millis(10));
                if runtime.click_travel.is_some() || !runtime.native_focus_is(ids[3]) {
                    return Err("Background repair survived a stale overview/Space".into());
                }
            }
        }
    }
    println!(
        "PASS: background repair cancels on external focus, stale identities/overviews/Spaces and retains destination focus after delayed compositor-order timeout"
    );
    Ok(())
}

fn right_edge_hover(runtime: &mut Runtime, tx: &Sender<Event>) -> Result<(), String> {
    runtime.clear_scroll();
    runtime.held = false;
    runtime.navigation_display = None;
    // AX focus can precede NSWorkspace activation in mixed-app fixtures.
    // Establish a read-only native baseline before asserting hover preserves it.
    let until = Instant::now() + Duration::from_secs(1);
    loop {
        runtime.reconcile(false);
        if runtime
            .engine
            .focused()
            .is_some_and(|id| runtime.native_focus_is(id))
        {
            break;
        }
        if Instant::now() >= until {
            return Err("Hover fixture did not establish native focus baseline".into());
        }
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
        }
    }
    let display = runtime.engine.display().ok_or("No hover fixture display")?;
    let frame = unsafe { CGDisplayBounds(display) };
    let edge = Point {
        x: frame.origin.x + frame.size.width - 0.5,
        y: frame.origin.y + frame.size.height / 2.,
    };
    let away = Point {
        x: frame.origin.x + frame.size.width / 2.,
        ..edge
    };
    let focused = runtime.engine.focused();
    let native_focus = focused.filter(|id| runtime.native_focus_is(*id));
    let send = |point, dragging| -> Result<(), String> {
        tx.send(Event::PointerMoved { point, dragging })
            .map_err(|error| error.to_string())
    };
    send(edge, true)?;
    runtime.drain();
    if runtime.ui.is_visible() || runtime.edge_display.is_some() {
        return Err("Ordinary window drag opened edge overview".into());
    }
    send(edge, false)?;
    runtime.drain();
    if !runtime.ui.is_visible()
        || runtime.ui.is_key()
        || runtime.edge_display != Some(display)
        || runtime.engine.display() != Some(display)
        || native_focus.is_some_and(|id| !runtime.native_focus_is(id))
    {
        return Err(format!(
            "Right-edge overview: visible={}, key={}, edge={:?}, display={:?} (expected {display}), native_focus_unchanged={}",
            runtime.ui.is_visible(),
            runtime.ui.is_key(),
            runtime.edge_display,
            runtime.engine.display(),
            native_focus.is_none_or(|id| runtime.native_focus_is(id)),
        ));
    }
    // Expire other ownership without moving the pointer: edge ownership must
    // outlive navigation feedback, modifier release, and discovery callbacks.
    runtime.linger_until = Some(Instant::now());
    tx.send(Event::OverviewHeld(false))
        .map_err(|error| error.to_string())?;
    runtime.drain();
    runtime.reconcile(false);
    if !runtime.ui.is_visible() || runtime.linger_until.is_some() {
        return Err("Stationary edge hover expired with feedback or a refresh".into());
    }
    let id = runtime
        .engine
        .context()
        .and_then(|ctx| ctx.focused.or(ctx.order.first().copied()))
        .ok_or("No hover thumbnail")?;
    let inside = runtime
        .ui
        .thumbnail_point(id, objc2_foundation::NSPoint::new(64., 41.))?;
    let inside = Point {
        x: inside.x,
        y: inside.y,
    };
    if !runtime.ui.contains_pointer(inside) {
        return Err("Native thumbnail point is outside hover strip".into());
    }
    let epoch = runtime.ui.visibility_epoch();
    send(inside, false)?;
    runtime.drain();
    runtime.linger_until = Some(Instant::now());
    runtime.drain();
    runtime.reconcile(false);
    if !runtime.ui.is_visible()
        || runtime.edge_display != Some(display)
        || runtime.ui.visibility_epoch() != epoch
        || runtime.engine.focused() != focused
        || native_focus.is_some_and(|id| !runtime.native_focus_is(id))
    {
        return Err(format!(
            "Moving from edge onto thumbnails lost stationary hover or native focus: visible={}, owner={:?}/{display}, epoch={}/{epoch}, recorded={:?}/{focused:?}, native={native_focus:?}/{}",
            runtime.ui.is_visible(),
            runtime.edge_display,
            runtime.ui.visibility_epoch(),
            runtime.engine.focused(),
            native_focus.is_none_or(|id| runtime.native_focus_is(id))
        ));
    }
    send(away, false)?;
    runtime.drain();
    if !runtime.ui.is_visible() || runtime.linger_until.is_none() {
        return Err("Leaving edge lost the dismissal grace period".into());
    }
    let scans = runtime.discovery.scan_count;
    send(inside, false)?;
    runtime.drain();
    if runtime.linger_until.is_some()
        || runtime.discovery.scan_count != scans
        || runtime.ui.visibility_epoch() != epoch
    {
        return Err(
            "Thumbnail re-entry retained a dismissal deadline or restarted overview discovery"
                .into(),
        );
    }
    // Click navigation must survive pointer ownership changing on re-entry.
    let target = runtime
        .engine
        .context()
        .and_then(|ctx| ctx.order.iter().copied().find(|window| *window != id))
        .ok_or("No second hover click fixture")?;
    runtime.click_travel = Some(ClickTravel {
        sequence: ClickSequence::new(&runtime.engine, target).ok_or("No hover click fixture")?,
        due: Instant::now() + Duration::from_secs(5),
        background: None,
    });
    send(away, false)?;
    runtime.drain();
    send(inside, false)?;
    runtime.drain();
    if runtime.click_travel.is_none() {
        return Err("Thumbnail hover interrupted click navigation".into());
    }
    runtime.click_travel = None;
    send(away, false)?;
    runtime.drain();
    // Re-enter before expiry and ensure the obsolete deadline cannot hide it.
    send(edge, false)?;
    runtime.drain();
    runtime.linger_until = Some(Instant::now());
    runtime.drain();
    if !runtime.ui.is_visible() {
        return Err("Old dismissal deadline hid a re-entered edge overview".into());
    }
    runtime.held = true;
    send(away, false)?;
    runtime.drain();
    runtime.linger_until = Some(Instant::now());
    runtime.drain();
    if !runtime.ui.is_visible() {
        return Err("Leaving edge hid a keyboard-held overview".into());
    }
    tx.send(Event::OverviewHeld(false))
        .map_err(|error| error.to_string())?;
    runtime.drain();
    if runtime.ui.is_visible() || runtime.edge_display.is_some() {
        return Err("Overview remained open after all holds ended".into());
    }
    hover_scrolling(runtime, tx, edge, inside, away)?;
    println!(
        "PASS: right-edge hover opens its monitor without focus, survives edge-to-thumbnail hover, stationary feedback expiry/modifier release/refresh/re-entry, respects keyboard holds, dismisses on exit, and ignores window drags"
    );
    Ok(())
}

fn hover_scrolling(
    runtime: &mut Runtime,
    tx: &Sender<Event>,
    edge: Point,
    inside: Point,
    away: Point,
) -> Result<(), String> {
    use objc2_foundation::NSPoint;
    let point = |p: Point| NSPoint::new(p.x, p.y);
    let mut queue = Queue {
        tx: tx.clone(),
        source: null(),
        bindings: runtime.config.bindings.clone(),
        shortcuts: ShortcutRouter::default(),
        runtime: null_mut(),
        scroll_modifiers: runtime.config.overlay_modifiers,
        overview_held: false,
        wheel: WheelRouter::default(),
        clock: Instant::now(),
        clicks: runtime.ui.clicks.clone(),
    };
    if queue.scroll_held(Modifiers::default(), point(inside)) {
        return Err("Hidden thumbnail region captured scrolling".into());
    }
    tx.send(Event::PointerMoved {
        point: edge,
        dragging: false,
    })
    .map_err(|e| e.to_string())?;
    tx.send(Event::PointerMoved {
        point: inside,
        dragging: false,
    })
    .map_err(|e| e.to_string())?;
    runtime.drain();
    for (index, continuous) in [false, true].into_iter().enumerate() {
        let held = queue.scroll_held(Modifiers::default(), point(inside));
        if !held || queue.overview_held || runtime.held {
            return Err("Hover scrolling required or fabricated a modifier hold".into());
        }
        let before = runtime.engine.focused();
        let direction = if runtime
            .engine
            .context()
            .is_some_and(|ctx| ctx.order.last().copied() == before)
        {
            -1
        } else {
            1
        };
        let mut expected = ScrubSession::new(&runtime.engine, Axis::Vertical)
            .ok_or("Missing hover scroll fixture")?;
        expected.update(direction);
        let raw = -(direction as f64) * if continuous { 90. } else { 1. };
        let time = index as f64;
        let WheelRoute::Navigate(delta) = queue.wheel.route(
            held,
            raw,
            continuous,
            0,
            if continuous { 1 } else { 0 },
            time,
        ) else {
            return Err("Hovered thumbnail scroll passed to the underlying application".into());
        };
        tx.send(Event::Wheel(delta, time))
            .map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.advance_scroll(Instant::now() + Duration::from_millis(112));
        if runtime.engine.focused() != expected.selected() || !runtime.ui.is_visible() {
            return Err(
                "Hover wheel/two-finger packet missed native selection or dismissed overview"
                    .into(),
            );
        }
        let expected = expected
            .selected()
            .ok_or("Hover scroll selection missing")?;
        let until = Instant::now() + Duration::from_millis(800);
        while !runtime.native_focus_is(expected) {
            if Instant::now() >= until {
                return Err("Hover scrolling did not change actual native focus".into());
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
            }
        }
        if continuous
            && queue.wheel.route(held, raw, true, 1, 0, time + 0.02) != WheelRoute::Consume
        {
            return Err("Hover scrolling replayed native trackpad momentum".into());
        }
    }
    tx.send(Event::PointerMoved {
        point: away,
        dragging: false,
    })
    .map_err(|e| e.to_string())?;
    runtime.drain();
    let held = queue.scroll_held(Modifiers::default(), point(away));
    if held || queue.wheel.route(held, -1., false, 0, 0, 1.1) != WheelRoute::Pass {
        return Err(
            "Wheel outside thumbnail region was intercepted during dismissal feedback".into(),
        );
    }
    if queue.wheel.route(held, -90., true, 1, 0, 1.1) != WheelRoute::Consume
        || queue.wheel.route(held, -90., true, 0, 1, 1.2) != WheelRoute::Pass
    {
        return Err("Hover exit leaked owned momentum or captured a fresh outside scroll".into());
    }
    runtime.linger_until = Some(Instant::now());
    runtime.drain();
    if queue.scroll_held(Modifiers::default(), point(inside)) || runtime.ui.is_visible() {
        return Err("Dismissed overview retained scroll capture".into());
    }
    println!(
        "PASS: hovered thumbnail region routes mouse wheel and two-finger scrolling without modifiers, changes real native focus, consumes owned momentum, and passes fresh outside/hidden-region scrolling"
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
        pending_swipe: None,
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
        edge_display: None,
        touch_overview: None,
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

fn contact_overview(runtime: &mut Runtime, tx: &Sender<Event>) -> Result<(), String> {
    runtime.clear_scroll();
    runtime.held = false;
    runtime.edge_display = None;
    runtime.navigation_display = None;
    runtime.render();
    let original = runtime.engine.focused();
    let frames: Vec<_> = runtime
        .engine
        .context()
        .ok_or("No contact fixture")?
        .order
        .iter()
        .map(|id| (*id, frame(runtime.discovery.handles[id].element.0)))
        .collect();
    tx.send(Event::TouchOverview(93, true))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    runtime.ui.verify_onscreen()?;
    let epoch = runtime.ui.visibility_epoch();
    runtime.advance_scroll(Instant::now() + Duration::from_secs(2));
    if !runtime.ui.is_visible()
        || runtime.gesture.is_some()
        || runtime.engine.focused() != original
        || original.is_some_and(|id| !runtime.native_focus_is(id))
        || frames
            .iter()
            .any(|(id, before)| frame(runtime.discovery.handles[id].element.0) != *before)
    {
        return Err(
            "Stationary contact changed focus/geometry or failed to hold the overview".into(),
        );
    }
    tx.send(Event::TouchOverview(94, true))
        .map_err(|e| e.to_string())?;
    tx.send(Event::TouchOverview(94, false))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    if !runtime.ui.is_visible() {
        return Err("Another device closed the held overview".into());
    }
    tx.send(Event::TouchOverview(93, false))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.ui.is_visible() || runtime.engine.focused() != original {
        return Err("Contact-only release did not dismiss without navigation".into());
    }
    tx.send(Event::TouchOverview(93, true))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    let epoch = runtime.ui.visibility_epoch().max(epoch);
    let mut expected =
        ScrubSession::new(&runtime.engine, Axis::Vertical).ok_or("No contact swipe fixture")?;
    let direction = if runtime
        .engine
        .context()
        .is_some_and(|c| c.order.last().copied() == original)
    {
        -1
    } else {
        1
    };
    expected.update(direction);
    tx.send(Event::Gesture(93, GestureEvent::Begin(Axis::Vertical), 1.))
        .map_err(|e| e.to_string())?;
    tx.send(Event::Gesture(93, GestureEvent::Update(direction), 1.05))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.engine.focused() != expected.selected() || runtime.ui.visibility_epoch() != epoch {
        return Err(
            "Contact-to-swipe transition changed overview identity or missed selection".into(),
        );
    }
    tx.send(Event::Gesture(
        93,
        GestureEvent::End { cancelled: false },
        1.1,
    ))
    .map_err(|e| e.to_string())?;
    tx.send(Event::TouchOverview(93, false))
        .map_err(|e| e.to_string())?;
    runtime.drain();
    if !runtime.ui.is_visible() || runtime.linger_until.is_none() {
        return Err("Swipe release lost normal overview feedback".into());
    }
    runtime.clear_scroll();
    tx.send(Event::TouchOverview(93, true))
        .map_err(|e| e.to_string())?;
    tx.send(Event::NativeGesture).map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.touch_overview.is_some() || runtime.ui.is_visible() {
        return Err("Fourth-finger handoff retained contact preview".into());
    }
    runtime.space_until = Some(Instant::now());
    runtime.drain();
    runtime.clear_scroll();
    runtime.render();
    println!(
        "PASS: stationary three-contact overview preserves native focus/geometry, stays visible while held, dismisses on lift, transitions to swiping without flicker, and yields to native gestures"
    );
    Ok(())
}

fn rapid_swipes(runtime: &mut Runtime, tx: &Sender<Event>, ids: &[u64]) -> Result<(), String> {
    use whalu::gestures::{Contact, SwipeRecognizer};
    runtime.clear_scroll();
    runtime.held = false;
    runtime.edge_display = None;
    runtime.navigation_display = runtime.engine.display();
    runtime.apply(vec![Effect::Focus(ids[0])]);
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
    }
    runtime.reconcile(false);
    runtime.deadline = None;
    let scans = runtime.discovery.scan_count;
    let mut recognizer = SwipeRecognizer::default();
    let mut slowest = Duration::ZERO;
    for (stroke, direction) in [1., 1., 1., -1.].into_iter().enumerate() {
        let t = stroke as f64 * 0.12;
        let mut expected =
            ScrubSession::new(&runtime.engine, Axis::Vertical).ok_or("No rapid swipe session")?;
        expected.update(direction as i32);
        for (dt, distance) in [(0., 0.), (0.02, 0.02), (0.05, 0.04)] {
            let contacts: Vec<_> = (0..3)
                .map(|id| Contact {
                    id,
                    x: 0.3 + id as f64 * 0.08,
                    y: 0.4 + direction * distance,
                })
                .collect();
            let started = Instant::now();
            for event in recognizer.feed(t + dt, &contacts) {
                tx.send(Event::Gesture(92, event, t + dt))
                    .map_err(|e| e.to_string())?;
            }
            runtime.drain();
            slowest = slowest.max(started.elapsed());
        }
        if runtime.engine.focused() != expected.selected() || runtime.gesture.is_none() {
            return Err("Rapid short swipe waited for finger lift to select".into());
        }
        // Production AX main/focused-window and workspace activation callbacks
        // all report focus-only changes. Their burst must not schedule a census.
        for _ in 0..4 {
            tx.send(Event::FocusChanged).map_err(|e| e.to_string())?;
        }
        runtime.drain();
        for event in recognizer.feed(t + 0.09, &[]) {
            tx.send(Event::Gesture(92, event, t + 0.09))
                .map_err(|e| e.to_string())?;
        }
        runtime.drain();
        if runtime.discovery.scan_count != scans || runtime.deadline.is_some() {
            return Err("Rapid swipe focus acknowledgments scheduled a full discovery scan".into());
        }
    }
    let expected = runtime.engine.focused().ok_or("No rapid swipe selection")?;
    let until = Instant::now() + Duration::from_millis(800);
    while !runtime.native_focus_is(expected) {
        if Instant::now() >= until {
            return Err("Rapid swipe burst did not reach real native focus".into());
        }
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
        }
    }
    // Invalid cohort data must still fall back to discovery; structural dirty
    // events keep their original census path regardless of focus notifications.
    runtime.discovery.invalidate_navigation();
    tx.send(Event::FocusChanged).map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.deadline.is_none() {
        return Err("Invalid focus cache did not request discovery".into());
    }
    runtime.deadline = Some(Instant::now());
    runtime.drain();
    if runtime.discovery.scan_count == scans {
        return Err("Invalid focus cache skipped discovery".into());
    }
    runtime.clear_scroll();
    runtime.render();
    println!(
        "PASS: four rapid recognized strokes select before lift without census scans, retain real native focus, and reject invalid caches; slowest fixture input turn {:.2} ms",
        slowest.as_secs_f64() * 1000.
    );
    Ok(())
}

fn post_space_swipe(runtime: &mut Runtime, tx: &mpsc::Sender<Event>) -> Result<(), String> {
    for release_before_settlement in [true, false] {
        runtime.clear_scroll();
        runtime.held = false;
        runtime.edge_display = None;
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.gesture_event(91, GestureEvent::Begin(Axis::Vertical), 0.08);
        let offset = if runtime
            .engine
            .context()
            .is_some_and(|c| c.order.last().copied() == runtime.engine.focused())
        {
            -1
        } else {
            1
        };
        runtime.gesture_event(91, GestureEvent::Update(offset), 0.12);
        // Hold the fallback guard to exercise a stroke that ends mid-animation;
        // the separate native settlement check below uses Dock's real state.
        runtime
            .pending_swipe
            .as_mut()
            .ok_or("First post-Space swipe was discarded")?
            .poll = Instant::now() + Duration::from_secs(2);
        let pending = runtime
            .pending_swipe
            .as_ref()
            .ok_or("First post-Space swipe was discarded")?;
        let space = pending.space;
        let display = pending.display;
        let original = runtime.engine.focused();
        let mut expected_session = ScrubSession::new(&runtime.engine, Axis::Vertical)
            .ok_or("Missing post-Space fixture session")?;
        expected_session.update(offset);
        let expected = expected_session.selected();
        if !runtime.engine.frozen || runtime.space_until.is_none() || runtime.gesture.is_some() {
            return Err("Post-Space swipe bypassed the native focus guard".into());
        }
        runtime.ui.verify_onscreen()?;
        if release_before_settlement {
            tx.send(Event::Gesture(
                91,
                GestureEvent::End { cancelled: false },
                0.16,
            ))
            .map_err(|e| e.to_string())?;
            runtime.drain();
        }
        if runtime.engine.focused() != original {
            return Err("Deferred swipe focused a window during the native transition".into());
        }
        runtime.space_until = Some(Instant::now());
        runtime.drain();
        if runtime.engine.focused() != expected
            || runtime.pending_swipe.is_some()
            || runtime.overview_space != Some((display, space))
            || !runtime.ui.is_visible()
        {
            return Err("First post-Space swipe did not resume at settlement".into());
        }
        if let Some(expected) = expected {
            let until = Instant::now() + Duration::from_millis(800);
            while !runtime.native_focus_is(expected) {
                if Instant::now() >= until {
                    return Err("Resumed swipe did not change real native focus".into());
                }
                unsafe {
                    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, false);
                }
            }
        }
        if !release_before_settlement {
            tx.send(Event::Gesture(
                91,
                GestureEvent::End { cancelled: false },
                0.9,
            ))
            .map_err(|e| e.to_string())?;
            runtime.drain();
        }
        // A new native gesture/Space notification must cancel deferred input,
        // including an already released stroke, without replaying its focus.
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        runtime.drain();
        runtime.gesture_event(91, GestureEvent::Begin(Axis::Vertical), 1.);
        runtime.gesture_event(91, GestureEvent::Update(-1), 1.1);
        if runtime.pending_swipe.is_none() {
            return Err("Missing deferred cancellation fixture".into());
        }
        tx.send(Event::NativeGesture).map_err(|e| e.to_string())?;
        tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
        runtime.drain();
        if runtime.pending_swipe.is_some() {
            return Err("Rapid Space change retained a previous swipe".into());
        }
        runtime.space_until = Some(Instant::now());
        runtime.drain();
        if runtime.engine.focused() != expected {
            return Err("Cancelled deferred swipe restored old focus".into());
        }
        runtime.clear_scroll();
        runtime.render();
    }
    if let Some(display) = runtime.engine.display() {
        if super::spaces::is_animating(display) == Some(false) {
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            tx.send(Event::Gesture(91, GestureEvent::Begin(Axis::Vertical), 2.))
                .map_err(|e| e.to_string())?;
            tx.send(Event::Gesture(91, GestureEvent::Update(-1), 2.1))
                .map_err(|e| e.to_string())?;
            runtime.drain();
            if runtime.space_until.is_some()
                || runtime.pending_swipe.is_some()
                || runtime.gesture.is_none()
            {
                return Err("Settled native Space still waited for the fallback timeout".into());
            }
            runtime.gesture_event(91, GestureEvent::End { cancelled: false }, 2.2);
            runtime.clear_scroll();
            runtime.render();
        }
    }
    println!(
        "PASS: first post-Space swipe shows native onscreen thumbnails immediately, retains held/released intent through settlement, changes real focus, and cancels on rapid Space interruption"
    );
    Ok(())
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
        runtime.ui.verify_onscreen()?;
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
        let fixture_window = runtime.discovery.handles[&id]
            .quartz_id
            .ok_or("No fixture identity")?;
        let original_membership = super::spaces::window_space(fixture_window)?;
        if super::spaces::bind_overview(fixture_window, source_space).is_ok()
            || super::spaces::window_space(fixture_window)? != original_membership
        {
            return Err("Overview rebind accepted or moved a foreign fixture window".into());
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
        contact_overview(&mut runtime, &tx)?;
        // Exercise the first three-finger stroke immediately after a real Space
        // return, including a lift before the native guard has expired.
        post_space_swipe(&mut runtime, &tx)?;
        // A long-lived hidden panel must survive fullscreen Space creation and
        // deletion, rather than merely following transfers while already open.
        runtime.space_until = Some(Instant::now());
        runtime.drain();
        let element = runtime.discovery.handles[&id].element.clone();
        for cycle in 0..4 {
            refocus(&runtime)?;
            let code =
                AXUIElementSetAttributeValue(element.0, string("AXFullScreen").0, kCFBooleanTrue);
            if code != 0 {
                return Err(format!("Could not enter fixture fullscreen: AX {code}"));
            }
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.03, false);
                if bool_attr(element.0, "AXFullScreen")
                    && super::spaces::active_space(
                        runtime.engine.display().ok_or("No fullscreen display")?,
                    ) != Some(source_space)
                {
                    break;
                }
                if Instant::now() >= until {
                    return Err("Fixture fullscreen entry timed out".into());
                }
            }
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            runtime.drain();
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.8, false);
            let code =
                AXUIElementSetAttributeValue(element.0, string("AXFullScreen").0, kCFBooleanFalse);
            if code != 0 {
                return Err(format!("Could not exit fixture fullscreen: AX {code}"));
            }
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.03, false);
                if !bool_attr(element.0, "AXFullScreen")
                    && super::spaces::active_space(
                        runtime.engine.display().ok_or("No restored display")?,
                    ) == Some(source_space)
                {
                    break;
                }
                if Instant::now() >= until {
                    return Err("Fixture fullscreen exit timed out".into());
                }
            }
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.8, false);
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            runtime.space_until = Some(Instant::now());
            runtime.drain();
            refocus(&runtime)?;
            if !super::spaces::move_adjacent(element.0, quartz, direction)? {
                return Err("No destination after fullscreen".into());
            }
            refocus(&runtime)?;
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            runtime.space_until = Some(Instant::now());
            runtime.drain();
            let before_open = frame(element.0);
            tx.send(Event::OverviewHeld(true))
                .map_err(|e| e.to_string())?;
            runtime.drain();
            runtime.overlay_deadline = None;
            runtime.render();
            runtime
                .ui
                .verify_onscreen()
                .map_err(|e| format!("After fullscreen cycle {cycle}: {e}"))?;
            if runtime.ui.is_key()
                || !runtime.native_focus_is(id)
                || frame(element.0) != before_open
            {
                return Err("Overview rebinding changed native focus or fixture geometry".into());
            }
            tx.send(Event::OverviewHeld(false))
                .map_err(|e| e.to_string())?;
            runtime.drain();
            refocus(&runtime)?;
            if !super::spaces::move_adjacent(element.0, quartz, !direction)? {
                return Err("No return after fullscreen".into());
            }
            refocus(&runtime)?;
            tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
            runtime.space_until = Some(Instant::now());
            runtime.drain();
        }
        println!(
            "PASS: hidden overview remains on-screen after four fullscreen Space creation/deletion cycles and desktop returns"
        );
        println!(
            "PASS: five Runtime Space moves with modifiers continuously held, unchanged cursor, visible non-key overview, refreshed destination lists, preserved focus/geometry, and source-window isolation"
        );
        Ok(())
    }
}
