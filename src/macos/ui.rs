use super::{Event, discovery::Discovery, enqueue, previews};
use magnetic::{core::Engine, gestures::Axis};
use objc2::{
    AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::*;
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
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
        self.panel.orderOut(None);
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
        let limit = (((b.size.height - 40.) / 108.).floor() as usize).clamp(1, 10);
        let selected = ctx
            .focused
            .and_then(|id| ctx.order.iter().position(|i| i == &id))
            .unwrap_or(0);
        let start = selected
            .saturating_sub(limit / 2)
            .min(ctx.order.len().saturating_sub(limit));
        let count = ctx.order.len().min(limit);
        let height = (count as f64 * 108. + 24.).min(b.size.height - 16.);
        let width = 176.;
        self.panel.setFrame_display(
            rect(
                b.origin.x,
                b.origin.y + (b.size.height - height) / 2.,
                width,
                height,
            ),
            false,
        );
        let root = &self.content;
        root.setFrame(rect(0., 0., width, height));
        for view in root.subviews() {
            view.removeFromSuperview();
        }
        let capture_allowed = previews::available();
        let mut targets = vec![];
        let mut sources = self.sources.borrow_mut();
        for (index, id) in ctx.order.iter().skip(start).take(limit).enumerate() {
            let Some(w) = engine.windows.get(id) else {
                continue;
            };
            let active = ctx.focused == Some(*id);
            let x = if active { 24. } else { 18. };
            let y = height - 96. - index as f64 * 108.;
            let card = NSView::initWithFrame(NSView::alloc(mtm), rect(x, y, 128., 82.));
            card.setWantsLayer(true);
            card.setClipsToBounds(false);
            let layer = card.layer().unwrap();
            {
                layer.setCornerRadius(10.);
                layer.setBackgroundColor(Some(
                    &NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.14, 0.18, 0.96).CGColor(),
                ));
                layer.setBorderWidth(if active { 2.5 } else { 1. });
                layer.setBorderColor(Some(
                    &if active {
                        NSColor::systemBlueColor()
                    } else {
                        NSColor::colorWithWhite_alpha(1., 0.4)
                    }
                    .CGColor(),
                ));
                // AppKit owns backing-layer shadow attributes; set the view's native
                // shadow so preview refreshes do not reset it.
                layer.setMasksToBounds(false);
                let silhouette = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    card.bounds(),
                    10.,
                    10.,
                );
                layer.setShadowPath(Some(&silhouette.CGPath()));
            }
            let shadow = NSShadow::new();
            shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0., 0.22)));
            shadow.setShadowBlurRadius(6.);
            shadow.setShadowOffset(NSSize::new(0., -2.));
            card.setShadow(Some(&shadow));
            root.addSubview(&card);
            let source = sources.get_mut(id);
            if let Some(source) = source.as_ref().filter(|s| s.image.is_some()) {
                let image =
                    NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(4., 4., 120., 74.));
                image.setImage(source.image.as_deref());
                image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                image.setWantsLayer(true);
                {
                    let l = image.layer().unwrap();
                    l.setCornerRadius(9.);
                    l.setMasksToBounds(true);
                }
                card.addSubview(&image);
            }
            card.setAccessibilityElement(true);
            card.setAccessibilityLabel(Some(&NSString::from_str(&format!(
                "{} — {}",
                w.app_name, w.title
            ))));
            let badge = NSView::initWithFrame(NSView::alloc(mtm), rect(x + 50., y - 12., 28., 28.));
            badge.setWantsLayer(true);
            {
                let l = badge.layer().unwrap();
                l.setCornerRadius(9.);
                l.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(0.96, 0.98).CGColor()));
                l.setShadowOpacity(0.35);
                l.setShadowRadius(3.);
            }
            let icon = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(2., 2., 24., 24.));
            if let Some(source) = source.as_ref() {
                icon.setImage(source.icon.as_deref());
            }
            // Diagnostic sample windows and apps without an icon get a native generic icon.
            if icon.image().is_none() {
                icon.setImage(
                    NSImage::imageNamed(&NSString::from_str("NSApplicationIcon")).as_deref(),
                );
            }
            icon.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
            badge.addSubview(&icon);
            root.addSubview(&badge);
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
            if !ctx.order.iter().skip(start).take(limit).any(|i| i == id) {
                source.image = None;
                source.updated = None;
            }
        }
        drop(sources);
        self.panel.orderFrontRegardless();
        previews::capture(targets);
    }
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
