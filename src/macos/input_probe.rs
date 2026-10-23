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
    let mut runtime = Runtime {
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
        mouse: false,
        dragging: false,
        manual_until: None,
        switched_space: false,
        pending_commands: vec![],
        space_until: None,
        space_retries: 0,
        paused: false,
        modal: false,
        recovery: SavedState::default(),
        restored: BTreeSet::new(),
        context_tracker: ContextTracker::default(),
        _lock: std::fs::File::open("/dev/null").map_err(|e| e.to_string())?,
    };
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
    } = runtime.ui.clicks.route(1, point)
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
        || runtime.ui.clicks.route(2, point) != ui::ClickRoute::Consume
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
        runtime.apply(vec![Effect::Focus(path[0])]);
        runtime.linger_until = Some(Instant::now() + Duration::from_millis(650));
        runtime.render();
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.4, false);
        }
        let point = runtime.ui.thumbnail_point(
            *path.last().unwrap(),
            objc2_foundation::NSPoint::new(64., 41.),
        )?;
        let ui::ClickRoute::Select {
            context,
            window,
            epoch,
        } = runtime.ui.clicks.route(1, point)
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
            let target = *path.last().unwrap();
            let handle = &runtime.discovery.handles[&target];
            let target_app = unsafe { Owned::new(AXUIElementCreateApplication(handle.pid)) }
                .ok_or("Missing target app")?;
            let actual = attr(target_app.0, "AXFocusedWindow")
                .is_some_and(|focused| unsafe { CFEqual(focused.0, handle.element.0) })
                && NSWorkspace::sharedWorkspace()
                    .frontmostApplication()
                    .is_some_and(|app| app.processIdentifier() == handle.pid);
            if runtime.engine.focused() != Some(target)
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
    }
    let context = runtime.engine.current;
    runtime.select_thumbnail(context, ids[3]);
    if runtime.engine.focused() != Some(ids[3])
        || runtime.ui.selected_thumbnail() != Some(ids[1])
        || runtime.click_travel.is_none()
    {
        return Err("Distant click did not focus immediately with visual stack travel".into());
    }
    runtime.select_thumbnail(context, ids[0]);
    runtime.advance_click(Instant::now() + Duration::from_secs(1));
    if runtime.engine.focused() != Some(ids[0]) || runtime.click_travel.is_some() {
        return Err("Second click did not replace the old destination".into());
    }
    runtime.select_thumbnail(context, ids[3]);
    runtime.contact_began();
    if runtime.engine.focused() != Some(ids[3]) || runtime.click_travel.is_none() {
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
    if runtime.click_travel.is_some() || runtime.engine.focused() != Some(ids[2]) {
        return Err(format!(
            "Keyboard input did not catch click travel: focused {:?}, expected {}, order {:?}, frozen {}, context {}",
            runtime.engine.focused(),
            ids[2],
            runtime.engine.context().map(|ctx| &ctx.order),
            runtime.engine.frozen,
            runtime.engine.current
        ));
    }
    println!(
        "PASS: immediate native focus with visual stack travel in both directions, stable feedback, ordinary pointer contact, retargeting, recognized-swipe and keyboard interruption"
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
