//! Small, on-demand per-window screenshots. No stream or disk cache is created.
use super::{Event, enqueue, ffi};
use block2::RcBlock;
use objc2::AnyThread;
use objc2_core_graphics::CGImage;
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
};

/// An immutable CGImage retained across the framework's callback queue. Keeping the
/// address opaque lets the main thread consume it without transferring NSImage.
#[derive(Debug)]
pub struct Image(usize);
impl Image {
    pub fn retain(image: &CGImage) -> Self {
        Self(unsafe { ffi::CFRetain((image as *const CGImage).cast()) } as usize)
    }
    pub fn get(&self) -> &CGImage {
        // The callback retains this immutable Core Graphics object until Drop.
        unsafe { &*(self.0 as *const CGImage) }
    }
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe { ffi::CFRelease(self.0 as ffi::Ref) }
    }
}
pub fn available() -> bool {
    unsafe { ffi::CGPreflightScreenCaptureAccess() }
}
pub fn request_access() {
    unsafe {
        ffi::CGRequestScreenCaptureAccess();
    }
}
pub fn capture(targets: Vec<(u64, u32)>) {
    capture_with(targets, |id, quartz, image| {
        enqueue(Event::PreviewReady(id, quartz, image));
    });
}
pub fn capture_with(
    targets: Vec<(u64, u32)>,
    completed: impl Fn(u64, u32, Option<Image>) + Send + Sync + 'static,
) {
    if targets.is_empty() || !available() {
        return;
    }
    let completed = std::sync::Arc::new(completed);
    let callback = RcBlock::new(
        move |content: *mut SCShareableContent, _: *mut NSError| unsafe {
            let windows = content.as_ref().map(|c| c.windows());
            for &(id, quartz) in &targets {
                let window = windows
                    .as_ref()
                    .and_then(|ws| ws.iter().find(|w| w.windowID() == quartz));
                let Some(window) = window else {
                    completed(id, quartz, None);
                    continue;
                };
                let filter = SCContentFilter::initWithDesktopIndependentWindow(
                    SCContentFilter::alloc(),
                    &window,
                );
                let config = SCStreamConfiguration::new();
                let frame = window.frame();
                let ratio = frame.size.height / frame.size.width.max(1.);
                config.setWidth(360);
                config.setHeight((360. * ratio).clamp(80., 600.) as usize);
                config.setShowsCursor(false);
                config.setCapturesAudio(false);
                config.setIgnoreShadowsSingleWindow(true);
                let completed = completed.clone();
                let done = RcBlock::new(move |image: *mut CGImage, _: *mut NSError| {
                    let retained = image.as_ref().map(Image::retain);
                    completed(id, quartz, retained);
                });
                SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                    &filter,
                    &config,
                    Some(&done),
                );
            }
        },
    );
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(true, false, &callback);
    }
}
