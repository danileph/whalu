# Magnetic

A Rust window manager for macOS with individual-window navigation, a temporary overview, and explicit snapping. Native Spaces, Mission Control, Dock, fullscreen, and application switching stay system-owned.

Grouping is deferred. Every window has its own entry in one Space-local stack. Snapping does not pair windows, and navigation focuses one window at a time. Existing saved pairings are ignored while Magnetic/Floating modes are retained.

## Controls

| Input | Action |
| --- | --- |
| Hold Control + Option | Show overview; release either modifier to dismiss |
| Click an overview thumbnail or app badge | Select and focus that window |
| Control + Option + Up / Down | Previous / next individual window |
| Control + Option + Left / Right | Previous / next window of the active application |
| Three fingers down / up | Previous / next window (natural scrolling) |
| Control + Option + mouse wheel | Scroll through individual windows |
| Three fingers left / right | Continuously select windows of the active application |
| Control + Option + Space | Toggle Magnetic / Floating |
| Control + Option + Command + Up | Snap to full usable display |
| Control + Option + Command + Left | Cycle left half → top-left quarter → bottom-left quarter |
| Control + Option + Command + Right | Cycle right half → top-right quarter → bottom-right quarter |
| Control + Option + Command + Down | Restore the rectangle from before snapping |

The former Shift + Control + Option group shortcuts and group-local H/J/K/L shortcuts are no longer captured. Control + arrows, Command + Tab, Option + M, two-finger scrolling, and native four-finger gestures pass through.

During a three-finger swipe, the overview stays open and focus follows your movement live. An ordinary swipe moves exactly one window, even when fast. To traverse more windows, keep your fingers down and continue moving after the overview appears; only then does speed-dependent acceleration engage. Lift after a fast sustained scroll for a stronger coast whose speed and distance follow your swipe velocity. Simply holding still does not unlock acceleration. A short swipe selects one window and holds the overview for 650 ms so the change stays visible. Rapid separate swipes keep that same overview open, advance one window per swipe, and restart the dismissal delay after the final swipe. Once sustained scrolling is active, holding near an edge continues through longer lists and gradually speeds up. A fourth contact cancels the session and restores the original visible window when still in the same Space. Gestures and mouse-wheel navigation stop at the list ends; keyboard navigation wraps. New input catches momentum immediately. Hold the configured `overlay_modifiers` (Control + Option by default) to use the mouse wheel; scrolling is consumed while navigating so the active app does not scroll too. Mouse-wheel bursts start with one window; longer continued scrolling enables slower traversal and a restrained coast of at most one extra window.

Snaps use exact full/half/quarter fractions of the usable display with no added gap. When an app enforces a minimum size, the accepted rectangle stays aligned to the requested right/bottom edges and within the display whenever it can fit. Snapping never enters native fullscreen.

Magnetic mode permits automatic position changes with fixed dimensions. Floating windows retain their rectangle. Explicit snaps can resize either mode; snapped rectangles stay fixed until another explicit snap or direct edit. The overview alone does not move windows, and automatic layout pauses during swiping.

## Install and run

```sh
./scripts/install.sh
```

Requires macOS 14.2 or later, Rust, and Apple's Command Line Tools. The script builds an ad-hoc signed bundle, installs `~/Applications/Magnetic.app`, and opens it. It does not install a login item.

On first launch, Magnetic retains a setup window and **M •** menu until permissions are ready. Enable the installed app in **System Settings → Privacy & Security → Accessibility**. If keyboard access is separately blocked, setup offers Input Monitoring settings. It starts automatically after permission is granted.

The overview is a vertical 3D wheel along the left edge of the focused window's display. The selected thumbnail stays at the display's vertical center, full size and facing forward, with a blue outline. Neighboring cards gradually shrink, recede, and tilt vertically toward the edges; both ends turn right around the Y axis. Selection changes glide smoothly from their current on-screen positions, including rapid reversal; opaque edge cards travel out of view without fading. First and last entries stay centered with empty space on the unused side. App icon badges stay in front of the preview borders and follow each card's perspective; individual shadows remain subtle. Cards float over a transparent background with no visible captions. The system Reduce Motion preference disables rotation and animated travel. Grouping remains disabled.

Window thumbnails require **Screen & System Audio Recording → Magnetic** in Privacy & Security. Choose **M → Enable window previews…** to request access. Magnetic takes small, individual-window snapshots on demand while the overview is open; it does not record audio, start a video stream, upload captures, or save them to disk. Without access, the same cards show app icons. Only the visible card set is cached in memory.

The **M** menu provides pause, refresh, preview access, configuration, reload, and quit. This development bundle's ad-hoc signature changes when rebuilt. If setup reports denied while Settings shows Magnetic enabled, remove that stale entry and add the current installed bundle again. The running GUI app's permission result is authoritative; terminal diagnostics can inherit the terminal's access.

## Configuration and recovery

Default location: `~/Library/Application Support/Magnetic/config.toml`.

Choose **M → Open configuration**, edit, then **M → Reload configuration**. Errors leave the previous valid configuration active. `--config /absolute/path/config.toml` selects another file.

```toml
gap = 8.0
overlay = true
overlay_delay_ms = 120
magnetic = true
gestures = true
ignored_apps = ["com.apple.QuickTimePlayerX"]
```

Legacy `overlay_brightness` values are accepted and ignored; generated configurations omit that removed setting.

`gap` affects automatic background exposure only. Set `magnetic = false` for navigation without automatic arrangement, or `gestures = false` for keyboard and modifier-wheel operation without three-finger navigation.

Bindings use physical macOS keycodes and exact modifier combinations. A supplied `bindings` array replaces the defaults. Removed actions (`group_next`, `group_previous`, and the former group-local `left`, `down`, `up`, `right`) must be removed from older custom configurations. Generate and check the current schema with:

```sh
~/.cargo/bin/cargo run -- --print-config > magnetic.example.toml
~/.cargo/bin/cargo run -- --check-config magnetic.example.toml
```

Window modes are atomically saved in `state.json` beside the default configuration. Recovery matches unique, nonempty app/title identities and never moves or resizes windows at startup. Ambiguous windows start Magnetic. Old grouping metadata is ignored and no longer written. Snap restore rectangles remain session-only.

After switching native Spaces without a focused window, the first navigation command enters the remembered visible window, or the first/last available entry. Commands during the native transition wait for it to settle.

## Platform boundaries

- Public APIs do not provide stable native Space IDs. The adapter uses AX/Quartz window identity and overlapping visibility cohorts. Sticky windows, identical windows across Spaces, and wholesale window replacement can confuse context identity. It does not move windows between Spaces or use private SkyLight APIs.
- Three-finger detection uses the experimental, read-only private MultitouchSupport framework. The public CGEvent tap consumes the three-contact scroll stream, including its trailing native momentum, so it cannot also scroll the active app. Ordinary two-finger and native four-finger input remain available, and no trackpad settings are changed. Keep system Space/Mission Control gestures on four fingers and three-finger dragging disabled. Keyboard navigation remains available if the listener is unavailable. ABI reference: [Hammerspoon touchdevice header](https://github.com/asmagill/hs._asm.undocumented.touchdevice/blob/master/MultitouchSupport.h).
- Dialogs, minimized/fullscreen windows, and windows without settable AX positions are excluded from active navigation. Focused modal/sheet/fullscreen UI suspends layout. Apps can reject size/position requests; windows larger than the whole display retain a reachable top-left corner.
- Display-aware snapping supports negative origins and usable bounds. Hardware validation covers this Mac, not every app, monitor configuration, or macOS release.

## Verification

```sh
~/.cargo/bin/cargo test
~/.cargo/bin/cargo clippy --all-targets -- -D warnings
~/.cargo/bin/cargo fmt --check
~/.cargo/bin/cargo run -- --native-smoke-test
~/.cargo/bin/cargo run -- --gesture-probe 10
~/.cargo/bin/cargo run -- --preview "$PWD/tmp/hud-preview.png"
```

Append `--hold` to `--preview` to watch a 25-second native wheel demonstration, including selection changes, reversal, and list ends. The preview also checks live compositor interpolation, interrupted reversal, stable card identity, selection centering, and 3D transform retention.

Tests cover independent window/app navigation, continuous swipe reversal and isolation, unfocused Spaces, legacy-state migration, fixed-size automatic layout, Floating ownership, exact snaps, minimum-size anchoring, restore, and configuration validation. The optional native smoke check manipulates only temporary child-process fixture windows and requires Accessibility. It verifies enumeration, real focus, continuous selection, fixed-size movement, snap/restore, minimum-size constraints, keyboard-tap creation, and a non-key HUD.

Scroll tests also cover natural direction, early three-contact ownership, partial release, native momentum isolation, modifier-wheel routing, bounded deceleration, reversal, and fresh-input interruption. Native fixture checks exercise short-swipe selection, kinetic focus, wheel bursts, and the 650 ms feedback/dismissal lifecycle without writing recovery state. Quit the running manager before the native smoke check so its automatic layout cannot move the test fixtures.

The runtime coalesces AX/workspace/display notifications on the main run loop and uses one-shot timers for bursts and transitions. No periodic window enumeration runs at idle. Input callbacks enqueue work without performing Accessibility calls.
