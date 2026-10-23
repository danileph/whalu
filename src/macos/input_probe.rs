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
    let fixture_pid = discovery.handles[&ids[0]].pid;
    let fixture_app_id = engine.windows[&ids[0]].app_id.clone();
    let ignored_apps = NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|app| app.processIdentifier() != fixture_pid)
        .map(|app| {
            app.bundleIdentifier()
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("pid:{}", app.processIdentifier()))
        })
        .filter(|id| id != &fixture_app_id)
        .collect();
    let mut runtime = Runtime {
        mtm,
        rx,
        config: Config {
            magnetic: false,
            ignored_apps,
            ..Config::default()
        },
        config_path: std::env::temp_dir().join("magnetic-input-probe.toml"),
        state_path: None,
        engine,
        discovery,
        ui,
        observers: BTreeMap::new(),
        tap: tap.clone(),
        gestures: None,
        gesture: None,
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
        paused: false,
        modal: false,
        recovery: SavedState::default(),
        restored: BTreeSet::new(),
        context_tracker: ContextTracker::default(),
        last_saved: None,
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
            return Err("Rapid swipe failed to change exactly one fixture without flicker".into());
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
    if runtime.engine.focused() != Some(ids[1])
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
    prepare(&mut runtime, ScrollSource::Trackpad(1))?;
    tx.send(Event::SpaceChanged).map_err(|e| e.to_string())?;
    runtime.drain();
    if runtime.gesture.is_some() || runtime.linger_until.is_some() || runtime.ui.is_visible() {
        return Err("Space change did not abandon navigation".into());
    }
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
