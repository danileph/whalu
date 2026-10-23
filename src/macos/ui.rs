use super::{Event, discovery::Discovery, enqueue, previews};
use magnetic::{
    core::Engine,
    gestures::Axis,
    overview::{Pose, Wheel},
};
use objc2::{
    AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::AnyObject, sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction, CATransaction, CATransform3D,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};

define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    pub struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    impl Delegate {
        #[unsafe(method(changed:))]
        fn changed(&self,_note:&NSNotification) {enqueue(Event::Dirty);}
        #[unsafe(method(spaceChanged:))]
        fn space_changed(&self,_note:&NSNotification) {enqueue(Event::SpaceChanged);}
        #[unsafe(method(pause:))]
        fn pause(&self,_sender:Option<&NSObject>) {enqueue(Event::TogglePause);}
        #[unsafe(method(refresh:))]
        fn refresh(&self,_sender:Option<&NSObject>) {enqueue(Event::Dirty);}
        #[unsafe(method(reload:))]
        fn reload(&self,_sender:Option<&NSObject>) {enqueue(Event::Reload);}
        #[unsafe(method(settings:))]
        fn settings(&self,_sender:Option<&NSObject>) {enqueue(Event::Settings);}
        #[unsafe(method(previews:))]
        fn previews(&self,_sender:Option<&NSObject>) {enqueue(Event::PreviewAccess);}
        #[unsafe(method(quit:))]
        fn quit(&self,_sender:Option<&NSObject>) {enqueue(Event::Quit);}
    }
);
impl Delegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

define_class!(
    #[unsafe(super=NSPanel)]
    #[thread_kind=MainThreadOnly]
    pub struct HudPanel;
    unsafe impl NSObjectProtocol for HudPanel {}
    impl HudPanel {
        #[unsafe(method(canBecomeKeyWindow))] fn can_become_key(&self)->bool {false}
        #[unsafe(method(canBecomeMainWindow))] fn can_become_main(&self)->bool {false}
    }
);

pub struct Ui {
    pub delegate: Retained<Delegate>,
    panel: Retained<HudPanel>,
    content: Retained<NSView>,
    sources: RefCell<BTreeMap<u64, Source>>,
    cards: RefCell<BTreeMap<u64, Card>>,
    visibility_epoch: Cell<u64>,
    pub clicks: ThumbnailClicks,
    _status: Retained<NSStatusItem>,
    pause: Retained<NSMenuItem>,
    gesture_label: Retained<NSMenuItem>,
}
impl Ui {
    pub fn new(mtm: MainThreadMarker) -> Self {
        unsafe {
            let delegate = Delegate::new(mtm);
            let panel: Retained<HudPanel> = msg_send![HudPanel::alloc(mtm),initWithContentRect:NSRect::new(NSPoint::new(0.,0.),NSSize::new(360.,200.)),styleMask:NSWindowStyleMask::Borderless|NSWindowStyleMask::NonactivatingPanel,backing:NSBackingStoreType::Buffered,defer:false];
            panel.setReleasedWhenClosed(false);
            panel.setLevel(25); // Above application windows, below system modal alerts.
            panel.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::IgnoresCycle
                    | NSWindowCollectionBehavior::Stationary,
            );
            panel.setHidesOnDeactivate(false);
            panel.setBecomesKeyOnlyIfNeeded(true);
            panel.setIgnoresMouseEvents(true);
            panel.setOpaque(false);
            panel.setBackgroundColor(Some(&NSColor::clearColor()));
            panel.setHasShadow(false);
            let content = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 176., 200.));
            content.setWantsLayer(true);
            content.setClipsToBounds(false);
            panel.setContentView(Some(&content));
            let status = NSStatusBar::systemStatusBar().statusItemWithLength(-1.);
            if let Some(button) = status.button(mtm) {
                button.setTitle(&NSString::from_str("M"));
                button.setToolTip(Some(&NSString::from_str("Magnetic Window Manager")));
            }
            let menu = NSMenu::new(mtm);
            let title = NSMenuItem::new(mtm);
            title.setTitle(&NSString::from_str("Magnetic · Control + Option"));
            title.setEnabled(false);
            menu.addItem(&title);
            let gesture_label = NSMenuItem::new(mtm);
            gesture_label.setTitle(&NSString::from_str("Three-finger gestures: off"));
            gesture_label.setEnabled(false);
            menu.addItem(&gesture_label);
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            let mut pause = None;
            for (text, action) in [
                ("Pause automatic positioning", sel!(pause:)),
                ("Refresh windows", sel!(refresh:)),
                ("Enable window previews…", sel!(previews:)),
                ("Open configuration", sel!(settings:)),
                ("Reload configuration", sel!(reload:)),
                ("Quit Magnetic", sel!(quit:)),
            ] {
                let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(text),
                    Some(action),
                    &NSString::from_str(""),
                );
                item.setTarget(Some(&delegate));
                menu.addItem(&item);
                if text.starts_with("Pause") {
                    pause = Some(item);
                }
            }
            status.setMenu(Some(&menu));
            Self {
                delegate,
                panel,
                content,
                sources: RefCell::new(BTreeMap::new()),
                cards: RefCell::new(BTreeMap::new()),
                visibility_epoch: Cell::new(0),
                clicks: ThumbnailClicks::default(),
                _status: status,
                pause: pause.unwrap(),
                gesture_label,
            }
        }
    }
    pub fn gesture_status(&self, active: bool) {
        self.gesture_label.setTitle(&NSString::from_str(if active {
            "Three-finger gestures: on"
        } else {
            "Three-finger gestures: unavailable or disabled"
        }));
    }
    pub fn visibility_epoch(&self) -> u64 {
        self.visibility_epoch.get()
    }
    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }
    pub fn is_key(&self) -> bool {
        self.panel.isKeyWindow()
    }
    pub fn snapshot(&self, path: &str) -> Result<(), String> {
        let view = self.panel.contentView().ok_or("HUD content view missing")?;
        self.panel.displayIfNeeded();
        let rep = view
            .bitmapImageRepForCachingDisplayInRect(view.bounds())
            .ok_or("Cannot allocate HUD bitmap")?;
        view.cacheDisplayInRect_toBitmapImageRep(view.bounds(), &rep);
        let data = unsafe {
            rep.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &objc2_foundation::NSDictionary::new(),
            )
        }
        .ok_or("Cannot encode HUD PNG")?;
        if !data.writeToFile_atomically(&NSString::from_str(path), true) {
            return Err("Cannot write HUD PNG".into());
        }
        Ok(())
    }
    pub fn hide(&self) {
        self.clicks.targets.borrow_mut().take();
        if self.panel.isVisible() {
            self.visibility_epoch.set(self.visibility_epoch.get() + 1);
        }
        self.panel.orderOut(None);
        self.cards.borrow_mut().retain(|_, card| {
            card.view.layer().unwrap().removeAllAnimations();
            card.body.layer().unwrap().removeAllAnimations();
            if card.retiring.is_some() {
                card.view.removeFromSuperview();
                false
            } else {
                true
            }
        });
    }
    pub fn paused(&self, paused: bool) {
        self.pause.setTitle(&NSString::from_str(if paused {
            "Resume automatic positioning"
        } else {
            "Pause automatic positioning"
        }));
    }
    pub fn show(&self, engine: &Engine) {
        self.show_navigation(engine, None);
    }
    /// Opt-in diagnostic: exercise real compositor interpolation and interrupted reversal.
    pub fn verify_wheel_animation(&self, engine: &mut Engine) -> Result<(), String> {
        let ids = engine.context().ok_or("No overview context")?.order.clone();
        if ids.len() < 2 {
            return Err("Wheel probe requires two windows".into());
        }
        let reduce_motion: bool = unsafe {
            msg_send![
                &*NSWorkspace::sharedWorkspace(),
                accessibilityDisplayShouldReduceMotion
            ]
        };
        engine.focus(ids[0]);
        self.hide();
        self.show(engine);
        pump(0.1);
        let identity = Retained::as_ptr(&self.cards.borrow()[&ids[0]].view);
        engine.focus(ids[1]);
        self.show(engine);
        pump(0.07);
        let layer = self.cards.borrow()[&ids[0]].view.layer().unwrap();
        if Retained::as_ptr(&self.cards.borrow()[&ids[0]].view) != identity {
            return Err("Selection rebuilt the existing card".into());
        }
        self.verify_thumbnail_clicks()?;
        let selected = self.cards.borrow()[&ids[1]].view.layer().unwrap();
        if (selected.position().y + 41. - self.content.bounds().size.height / 2.).abs() > 0.01 {
            return Err(format!(
                "Selected card is not centered: position {:?}, anchor {:?}, height {}",
                selected.position(),
                selected.anchorPoint(),
                self.content.bounds().size.height
            ));
        }
        if !reduce_motion {
            let mid = unsafe { layer.presentationLayer() }.ok_or("No live presentation layer")?;
            if (mid.position().y - layer.position().y).abs() < 1. {
                return Err("Wheel jumped to its destination without interpolation".into());
            }
            engine.focus(ids[0]);
            self.show(engine);
            let animation = unsafe { layer.animationForKey(&NSString::from_str("position")) }
                .ok_or("Reversal did not animate")?;
            let from: Retained<AnyObject> = unsafe { msg_send![&*animation, fromValue] };
            let point: NSPoint = unsafe { msg_send![&*from, pointValue] };
            if (point.y - mid.position().y).abs() > 2. {
                return Err("Reversal did not start at its current on-screen position".into());
            }
        }
        for id in [ids[ids.len() / 2], *ids.last().unwrap(), ids[ids.len() / 2]] {
            engine.focus(id);
            self.show(engine);
            pump(0.4);
        }
        for card in self.cards.borrow().values() {
            let layer = card.view.layer().unwrap();
            let pose = card.pose.unwrap();
            if layer.opacity() != 1. || card.body.layer().unwrap().opacity() != 1. {
                return Err("A wheel card is translucent".into());
            }
            if unsafe { card.badge.superview() }.as_deref() != Some(&*card.view)
                || unsafe { card.body.superview() }.as_deref() != Some(&*card.view)
                || card.badge.layer().unwrap().zPosition() <= card.body.layer().unwrap().zPosition()
                || layer.borderWidth() != 0.
            {
                return Err("Badge is not in front of the separate preview border".into());
            }
            let t = layer.transform();
            let w = 64. * t.m14 + 41. * t.m24 + t.m44;
            let center_x = layer.position().x + (64. * t.m11 + 41. * t.m21 + t.m41) / w;
            let center_y = layer.position().y + (64. * t.m12 + 41. * t.m22 + t.m42) / w;
            if (center_x - pose.x).abs() > 0.1
                || (center_y - self.content.bounds().size.height / 2. - pose.y).abs() > 0.1
            {
                return Err("Projected card center drifted during 3D rotation".into());
            }
            let expected = if reduce_motion { 0. } else { pose.pitch };
            if expected.abs() > 0.01 && layer.transform().m23.abs() < 0.01 {
                return Err("AppKit reset the card's 3D transform".into());
            }
            if let Some(visible) = unsafe { layer.presentationLayer() } {
                if (visible.transform().m23 - layer.transform().m23).abs() > 0.01
                    || (visible.opacity() - layer.opacity()).abs() > 0.01
                {
                    return Err(format!(
                        "Presentation transform/opacity did not settle: {:?} {:?}",
                        visible.transform(),
                        layer.transform()
                    ));
                }
                if (visible.position().y - layer.position().y).abs() > 0.1 {
                    return Err("Wheel did not settle at its target".into());
                }
            }
        }
        self.show(engine);
        if unsafe { layer.animationForKey(&NSString::from_str("position")) }.is_some() {
            return Err("Unchanged overview refresh restarted motion".into());
        }
        self.verify_thumbnail_clicks()?;
        let point = self.thumbnail_point(engine.focused().unwrap(), NSPoint::new(64., 41.))?;
        self.hide();
        if self.clicks.route(1, point) != ClickRoute::Pass {
            return Err("Hidden overview intercepted a click".into());
        }
        self.show(engine);
        println!(
            "PASS: opaque wheel cards interpolate, reverse continuously, remain centered, retain 3D transforms, and draw badges in front of borders"
        );
        Ok(())
    }
    /// Native diagnostic coordinates use the same live presentation tree as hit testing.
    pub fn thumbnail_point(&self, id: u64, local: NSPoint) -> Result<NSPoint, String> {
        let targets = self.clicks.targets.borrow();
        let map = targets.as_ref().ok_or("Overview is hidden")?;
        let (_, layer) = map
            .cards
            .iter()
            .find(|(window, _)| *window == id)
            .ok_or("Thumbnail is outside the visible working set")?;
        let visible = unsafe { layer.presentationLayer() }.unwrap_or_else(|| layer.clone());
        let root = unsafe { map.root.presentationLayer() }.unwrap_or_else(|| map.root.clone());
        let point = visible.convertPoint_toLayer(local, Some(&root));
        Ok(NSPoint::new(
            point.x + map.frame.origin.x,
            map.desktop_top - point.y - map.frame.origin.y,
        ))
    }
    fn verify_thumbnail_clicks(&self) -> Result<(), String> {
        let ids: Vec<_> = self
            .cards
            .borrow()
            .iter()
            .filter(|(_, card)| card.retiring.is_none())
            .map(|(id, _)| *id)
            .collect();
        let context = self.clicks.targets.borrow().as_ref().unwrap().context;
        for id in ids {
            for local in [NSPoint::new(64., 41.), NSPoint::new(64., -7.)] {
                let point = self.thumbnail_point(id, local)?;
                let frame = self.panel.frame();
                let desktop_top = NSScreen::screens(self.panel.mtm())
                    .firstObject()
                    .map_or(0., |screen| screen.frame().size.height);
                if !contains(frame, NSPoint::new(point.x, desktop_top - point.y)) {
                    continue;
                }
                let actual = self.clicks.route(1, point);
                if actual
                    != (ClickRoute::Select {
                        context,
                        window: id,
                    })
                {
                    return Err(format!(
                        "Animated thumbnail/badge {id} click missed: {actual:?}"
                    ));
                }
                // Even after moving outside the strip, drag/up belong to this click.
                let outside = NSPoint::new(-10000., -10000.);
                if self.clicks.route(6, outside) != ClickRoute::Consume
                    || self.clicks.route(2, outside) != ClickRoute::Consume
                    || self.clicks.route(1, outside) != ClickRoute::Pass
                    || self.clicks.route(2, outside) != ClickRoute::Pass
                {
                    return Err("Thumbnail click leaked drag/up or captured outside input".into());
                }
            }
        }
        let frame = self.panel.frame();
        let top = NSScreen::screens(self.panel.mtm())
            .firstObject()
            .map_or(0., |screen| screen.frame().size.height);
        if self.clicks.route(
            1,
            NSPoint::new(
                frame.origin.x + 1.,
                top - frame.origin.y - frame.size.height / 2.,
            ),
        ) != ClickRoute::Pass
        {
            return Err("Empty overview strip intercepted a click".into());
        }
        println!(
            "PASS: animated thumbnail/badge hit testing, empty-area passthrough and complete click ownership"
        );
        Ok(())
    }
    pub fn sync_sources(&self, discovery: &Discovery) {
        let mut sources = self.sources.borrow_mut();
        sources.retain(|id, _| discovery.handles.contains_key(id));
        for (&id, h) in &discovery.handles {
            let Some(quartz) = h.quartz_id else {
                continue;
            };
            let source = sources.entry(id).or_insert_with(|| Source::new(quartz));
            if source.quartz != quartz {
                *source = Source::new(quartz);
            }
            source.icon = h.app.icon();
        }
    }
    pub fn preview_ready(&self, id: u64, quartz: u32, image: Option<previews::Image>) {
        if let Some(source) = self
            .sources
            .borrow_mut()
            .get_mut(&id)
            .filter(|s| s.quartz == quartz)
        {
            source.pending = None;
            source.updated = Some(Instant::now());
            if let Some(image) = image {
                source.image = Some(NSImage::initWithCGImage_size(
                    NSImage::alloc(),
                    image.get(),
                    NSSize::new(0., 0.),
                ));
            }
        }
    }
    pub fn show_navigation(&self, engine: &Engine, _gesture: Option<Axis>) {
        let Some(ctx) = engine.context() else {
            self.hide();
            return;
        };
        if ctx.order.is_empty() {
            self.hide();
            return;
        }
        let mtm = self.panel.mtm();
        // Match the focused window's display, including displays with negative origins.
        let bounds = ctx
            .focused
            .and_then(|id| engine.windows.get(&id))
            .and_then(|w| engine.bounds.get(&w.display));
        let desktop_top = NSScreen::screens(mtm)
            .firstObject()
            .map_or(0., |s| s.frame().size.height);
        let screens = NSScreen::screens(mtm);
        let screen = bounds
            .and_then(|b| {
                screens.iter().find(|s| {
                    let f = s.frame();
                    let x = b.x + b.width / 2.;
                    let y = desktop_top - b.y - b.height / 2.;
                    x >= f.origin.x
                        && x < f.origin.x + f.size.width
                        && y >= f.origin.y
                        && y < f.origin.y + f.size.height
                })
            })
            .or_else(|| NSScreen::mainScreen(mtm));
        let Some(screen) = screen else {
            return;
        };
        let b = screen.visibleFrame();
        // A fixed display-relative center keeps first/last entries centered too.
        let height = (b.size.height - 16.).max(132.);
        let wheel = Wheel::new(height);
        let selected = ctx
            .focused
            .and_then(|id| ctx.order.iter().position(|i| i == &id))
            .unwrap_or(0);
        let start = selected.saturating_sub(wheel.radius + 1);
        let end = (selected + wheel.radius + 2).min(ctx.order.len());
        let frame = rect(
            b.origin.x,
            b.origin.y + (b.size.height - height) / 2.,
            176.,
            height,
        );
        let animate = self.panel.isVisible() && self.panel.frame() == frame;
        // Respect the system motion preference without requiring a second rendering path.
        let reduce_motion: bool = unsafe {
            msg_send![
                &*NSWorkspace::sharedWorkspace(),
                accessibilityDisplayShouldReduceMotion
            ]
        };
        self.panel.setFrame_display(frame, false);
        self.content.setFrame(rect(0., 0., 176., height));
        let mut cards = self.cards.borrow_mut();
        let direction = cards
            .iter()
            .find(|(_, card)| card.active)
            .and_then(|(id, _)| ctx.order.iter().position(|entry| entry == id))
            .map_or(1., |old| (selected as f64 - old as f64).signum());
        let now = Instant::now();
        cards.retain(|id, card| {
            if ctx.order[start..end].contains(id) {
                card.retiring = None;
                return true;
            }
            if !animate
                || card
                    .retiring
                    .is_some_and(|at| now.duration_since(at) >= Duration::from_millis(300))
            {
                card.view.removeFromSuperview();
                return false;
            }
            if card.retiring.is_none() {
                let side = card.pose.map_or(-direction, |pose| {
                    if pose.y.abs() < 1. {
                        -direction
                    } else {
                        -pose.y.signum()
                    }
                });
                card.place(
                    wheel.pose(side * (wheel.radius + 1) as f64),
                    height,
                    !reduce_motion,
                    false,
                    reduce_motion,
                );
                card.retiring = Some(now);
            }
            true
        });
        let capture_allowed = previews::available();
        let mut targets = vec![];
        let mut sources = self.sources.borrow_mut();
        for (index, id) in ctx.order.iter().enumerate().take(end).skip(start) {
            let Some(w) = engine.windows.get(id) else {
                continue;
            };
            let active = ctx.focused == Some(*id);
            let offset = index as f64 - selected as f64;
            let pose = wheel.pose(offset);
            let fresh = !cards.contains_key(id);
            let card = cards.entry(*id).or_insert_with(|| {
                let mut card = Card::new(mtm);
                self.content.addSubview(&card.view);
                // Start newly entering cards just outside the visible wheel.
                let entrance = if animate {
                    wheel.pose(
                        if offset == 0. {
                            direction
                        } else {
                            offset.signum()
                        } * (wheel.radius + 1) as f64,
                    )
                } else {
                    pose
                };
                card.place(entrance, height, false, active, reduce_motion);
                card
            });
            card.view
                .setAccessibilityLabel(Some(&NSString::from_str(&format!(
                    "{}{} — {}",
                    if active { "Selected: " } else { "" },
                    w.app_name,
                    w.title
                ))));
            // Image callbacks update existing image views, never the card identities or
            // running motion. Only a changed target retargets an animation.
            let source = sources.get_mut(id);
            card.image
                .setImage(source.as_ref().and_then(|s| s.image.as_deref()));
            card.icon
                .setImage(source.as_ref().and_then(|s| s.icon.as_deref()));
            if card.icon.image().is_none() {
                card.icon.setImage(
                    NSImage::imageNamed(&NSString::from_str("NSApplicationIcon")).as_deref(),
                );
            }
            if fresh || card.pose != Some(pose) || card.active != active || !animate {
                card.place(
                    pose,
                    height,
                    animate && !reduce_motion,
                    active,
                    reduce_motion,
                );
            }
            if let Some(source) = source {
                let now = Instant::now();
                if capture_allowed
                    && source
                        .pending
                        .is_none_or(|t| now.duration_since(t) > Duration::from_secs(10))
                    && source
                        .updated
                        .is_none_or(|t| now.duration_since(t) > Duration::from_secs(3))
                {
                    source.pending = Some(now);
                    targets.push((*id, source.quartz));
                }
            }
        }
        // Retain only a small in-memory thumbnail working set.
        for (id, source) in sources.iter_mut() {
            if !ctx.order[start..end].contains(id) {
                source.image = None;
                source.updated = None;
            }
        }
        drop(sources);
        *self.clicks.targets.borrow_mut() = Some(ClickMap {
            context: engine.current,
            frame,
            desktop_top,
            root: self.content.layer().unwrap(),
            cards: cards
                .iter()
                .filter(|(_, card)| card.retiring.is_none())
                .map(|(id, card)| (*id, card.view.layer().unwrap()))
                .collect(),
        });
        drop(cards);
        self.panel.orderFrontRegardless();
        previews::capture(targets);
    }
}

/// Shared only on the main run loop. The tap reads rendering state and enqueues
/// selection; it never borrows Runtime or performs Accessibility work.
#[derive(Clone, Default)]
pub(super) struct ThumbnailClicks {
    targets: Rc<RefCell<Option<ClickMap>>>,
    pressed: Rc<Cell<bool>>,
}
struct ClickMap {
    context: u64,
    frame: NSRect,
    desktop_top: f64,
    root: Retained<CALayer>,
    cards: Vec<(u64, Retained<CALayer>)>,
}
#[derive(Debug, PartialEq)]
pub(super) enum ClickRoute {
    Pass,
    Consume,
    Select { context: u64, window: u64 },
}
impl ThumbnailClicks {
    pub fn reset(&self) {
        self.pressed.set(false);
    }
    pub fn route(&self, kind: u32, point: NSPoint) -> ClickRoute {
        match kind {
            1 => {
                let target = self.hit(point);
                self.pressed.set(target.is_some());
                target.map_or(ClickRoute::Pass, |(context, window)| ClickRoute::Select {
                    context,
                    window,
                })
            }
            2 if self.pressed.replace(false) => ClickRoute::Consume,
            6 if self.pressed.get() => ClickRoute::Consume,
            _ => ClickRoute::Pass,
        }
    }
    fn hit(&self, point: NSPoint) -> Option<(u64, u64)> {
        let targets = self.targets.try_borrow().ok()?;
        let map = targets.as_ref()?;
        // Quartz uses top-left screen coordinates; AppKit uses bottom-left.
        let point = NSPoint::new(
            point.x - map.frame.origin.x,
            map.desktop_top - point.y - map.frame.origin.y,
        );
        if !contains(
            rect(0., 0., map.frame.size.width, map.frame.size.height),
            point,
        ) {
            return None;
        }
        let root = unsafe { map.root.presentationLayer() }.unwrap_or_else(|| map.root.clone());
        map.cards
            .iter()
            .filter_map(|(id, layer)| {
                let visible = unsafe { layer.presentationLayer() }.unwrap_or_else(|| layer.clone());
                let local = visible.convertPoint_fromLayer(point, Some(&root));
                // Include the badge extending below the thumbnail, but not the
                // empty strip or the shadow. Follow the current animated pose.
                (contains(rect(0., 0., 128., 82.), local)
                    || contains(rect(50., -12., 28., 28.), local))
                .then_some((*id, visible.zPosition()))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| (map.context, id))
    }
}
/// Views survive selection changes so Core Animation can retarget their live
/// presentation layers during fast swipes and reversals.
struct Card {
    view: Retained<NSView>,
    body: Retained<NSView>,
    badge: Retained<NSView>,
    image: Retained<NSImageView>,
    icon: Retained<NSImageView>,
    pose: Option<Pose>,
    active: bool,
    retiring: Option<Instant>,
}
impl Card {
    fn new(mtm: MainThreadMarker) -> Self {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 128., 82.));
        view.setWantsLayer(true);
        view.setClipsToBounds(false);
        view.setAccessibilityElement(true);
        view.layer().unwrap().setAnchorPoint(NSPoint::new(0., 0.));
        // The preview body and badge are siblings. A parent layer's border draws
        // over its children, so putting the badge inside the bordered body clips it.
        let body = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 128., 82.));
        body.setWantsLayer(true);
        body.setClipsToBounds(false);
        let layer = body.layer().unwrap();
        layer.setCornerRadius(10.);
        layer.setBackgroundColor(Some(
            &NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.14, 0.18, 1.).CGColor(),
        ));
        layer.setMasksToBounds(false);
        let silhouette =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(view.bounds(), 10., 10.);
        layer.setShadowPath(Some(&silhouette.CGPath()));
        let shadow = NSShadow::new();
        shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0., 0.22)));
        shadow.setShadowBlurRadius(6.);
        shadow.setShadowOffset(NSSize::new(0., -2.));
        body.setShadow(Some(&shadow));
        let image = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(4., 4., 120., 74.));
        image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        image.setWantsLayer(true);
        image.layer().unwrap().setCornerRadius(9.);
        image.layer().unwrap().setMasksToBounds(true);
        body.addSubview(&image);
        view.addSubview(&body);
        // Badge belongs to the card, so it follows the same perspective and scale.
        let badge = NSView::initWithFrame(NSView::alloc(mtm), rect(50., -12., 28., 28.));
        badge.setWantsLayer(true);
        let badge_layer = badge.layer().unwrap();
        badge_layer.setCornerRadius(9.);
        badge_layer.setZPosition(1.);
        badge_layer.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(0.96, 1.).CGColor()));
        let icon = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(2., 2., 24., 24.));
        icon.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        badge.addSubview(&icon);
        view.addSubview(&badge);
        CATransaction::commit();
        Self {
            view,
            body,
            badge,
            image,
            icon,
            pose: None,
            active: false,
            retiring: None,
        }
    }
    fn place(&mut self, pose: Pose, height: f64, animate: bool, active: bool, reduce_motion: bool) {
        let layer = self.view.layer().unwrap();
        let border = self.body.layer().unwrap();
        let properties = [
            (&layer, "position"),
            (&layer, "transform"),
            (&border, "borderWidth"),
            (&border, "borderColor"),
        ];
        let from = if animate {
            // Use the pixels currently on screen, including an interrupted animation.
            properties
                .iter()
                .map(|(target, key)| {
                    let visible =
                        unsafe { target.presentationLayer() }.unwrap_or_else(|| (*target).clone());
                    layer_value(&visible, key)
                })
                .collect::<Vec<_>>()
        } else {
            layer.removeAllAnimations();
            border.removeAllAnimations();
            vec![]
        };
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.view
            .setFrameOrigin(NSPoint::new(pose.x - 64., height / 2. + pose.y - 41.));

        let mut transform = CATransform3D::new_scale(1., 1., 1.);
        if !reduce_motion {
            transform.m34 = -1. / 600.;
            transform = transform
                .translate(0., 0., pose.depth)
                .rotate(pose.pitch, 1., 0., 0.)
                .rotate(pose.yaw, 0., 1., 0.);
        }
        // AppKit owns the backing layer's zero anchor. Conjugate the transform
        // around the card center instead of changing that anchor (which AppKit resets).
        transform = CATransform3D::new_translation(-64., -41., 0.)
            .concat(transform.scale(pose.scale, pose.scale, 1.))
            .concat(CATransform3D::new_translation(64., 41., 0.));
        layer.setTransform(transform);
        layer.setOpacity(1.);
        // Render the central card in front of receding neighbors.
        layer.setZPosition(if active { 1. } else { pose.depth });
        border.setBorderWidth(if active { 2.5 } else { 1. });
        border.setBorderColor(Some(
            &if active {
                NSColor::systemBlueColor()
            } else {
                NSColor::colorWithWhite_alpha(1., 0.4)
            }
            .CGColor(),
        ));
        CATransaction::commit();
        if animate {
            for ((target, key), from) in properties.iter().zip(from) {
                let to = layer_value(target, key);
                let key = NSString::from_str(key);
                let animation = CABasicAnimation::animationWithKeyPath(Some(&key));
                unsafe {
                    animation.setFromValue(from.as_deref());
                    animation.setToValue(to.as_deref());
                }
                animation.setDuration(0.28);
                animation.setTimingFunction(Some(
                    &CAMediaTimingFunction::functionWithControlPoints(0.22, 0.72, 0.18, 1.),
                ));
                target.addAnimation_forKey(&animation, Some(&key));
            }
        }
        self.pose = Some(pose);
        self.active = active;
    }
}
fn layer_value(layer: &CALayer, key: &str) -> Option<Retained<AnyObject>> {
    // Core Animation boxes each property in its matching NSValue/NSNumber/CGColor.
    unsafe { msg_send![layer, valueForKey: &*NSString::from_str(key)] }
}

struct Source {
    quartz: u32,
    icon: Option<Retained<NSImage>>,
    image: Option<Retained<NSImage>>,
    updated: Option<Instant>,
    pending: Option<Instant>,
}
impl Source {
    fn new(quartz: u32) -> Self {
        Self {
            quartz,
            icon: None,
            image: None,
            updated: None,
            pending: None,
        }
    }
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

fn pump(seconds: f64) {
    unsafe {
        super::ffi::CFRunLoopRunInMode(super::ffi::kCFRunLoopDefaultMode, seconds, false);
    }
}
