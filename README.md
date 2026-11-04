<p align="center">
  <img src="assets/Whalu-wordmark.png" width="760" alt="Whalu — frosted glass window panes with a silver orbit">
</p>

# Whalu

[Website](https://danileph.github.io/whalu/) · [Download for Mac](https://github.com/danileph/whalu/releases/latest/download/Whalu-macOS-universal.zip) · [Release notes](https://github.com/danileph/whalu/releases/latest)

Whalu's code is licensed under the [MIT License](LICENSE). See the [third-party notices](THIRD_PARTY_NOTICES.md) and the [demo icon credits](website/public/app-icons/README.md) for separately licensed material.

Whalu provides individual-window navigation, a temporary overview, stack reordering, and shortcuts for native macOS window tiling. macOS owns window sizes, tiling margins, multi-window arrangements, and previous-size restoration. Whalu never automatically repositions background windows.

Every window has its own entry in a Space-local stack. Reordering changes that list without changing focus or window geometry. The focused window can also move to the previous or next regular desktop on its display, and the desktop follows the window.

## Controls

| Action | Arrow shortcut | Vim-style / key |
| --- | --- | --- |
| Focus previous / next window | Control + Option + Up / Down | Control + Option + K / J |
| Focus next / previous window | Command + Tab / grave accent (`) | — |
| Switch to the last used window | Command + Esc | — |
| Focus previous / next window of the active app | Control + Option + Left / Right | Control + Option + H / L |
| Move focused window up in stack | Control + Option + Command + Up | Control + Option + Command + K |
| Move focused window down in stack | Control + Option + Command + Down | Control + Option + Command + J |
| Move focused window to previous Space | Control + Option + Command + Left | Control + Option + Command + H |
| Move focused window to next Space | Control + Option + Command + Right | Control + Option + Command + L |
| Native left half | Control + Command + Left | Control + Command + H |
| Native bottom half | Control + Command + Down | Control + Command + J |
| Native top half | Control + Command + Up | Control + Command + K |
| Native right half | Control + Command + Right | Control + Command + L |
| Native fill desktop | — | Control + Command + M |
| Native center window | — | Control + Command + I |
| Native restore previous size | — | Control + Command + N |
| Native arrange left and right | Control + Command + Shift + Left | Control + Command + Shift + H |
| Native arrange right and left | Control + Command + Shift + Right | Control + Command + Shift + L |
| Native arrange top and bottom | Control + Command + Shift + Up | Control + Command + Shift + K |
| Native arrange bottom and top | Control + Command + Shift + Down | Control + Command + Shift + J |

Hold Control + Option to show the overview; release either modifier to dismiss. Click a thumbnail or app badge, use Control + Option + mouse wheel, or swipe with three fingers to navigate. Vertical swipes select individual windows; horizontal swipes do not switch windows. Control + Option + arrows and H/J/K/L navigate windows; adding Command keeps the separate stack-edit and Space-transfer actions. Command + Tab moves down the window stack; Command + grave accent (`) moves up, replacing the native actions for those exact combinations. The overview opens immediately and stays visible while Command or either navigation key remains held. After a quick press and release, it stays visible for 650 ms, matching short three-finger swipes. Rapid separate presses navigate immediately, keep the same overview open, and restart that delay after the final release. Mouse-wheel navigation works while the shortcut session is held. The old mode-toggle shortcut is no longer captured. Control + arrows, two-finger scrolling outside overview navigation, and native four-finger gestures remain available.

Center first restores macOS’s pre-tiling size, then centers the window, so a filled window does not stay desktop-sized.

Command + Esc toggles between the two most recently focused windows in the active display and Space. It works across apps and between windows of the same app, and skips windows that are no longer visible. Each successful switch shows the overview for 650 ms when the overview is enabled.

Tiling shortcuts directly invoke the foreground app's native Window-menu leaf actions through Accessibility, locating their Fn + Control key equivalents (with Shift for arrangements). The menu bar and submenus stay closed. Actions are cached per process, and stale references are rediscovered after failures. A bounded background queue performs lookup and invocation so a busy target app cannot block Whalu’s input loop; requests are discarded if their focused window changes. Whalu does not synthesize Fn events or compute window rectangles. Native Fn + Control + R remains available alongside the custom Control + Command + N restore alias. Apps must support the corresponding native command. Repeating a half shortcut keeps that native half; the former quarter-cycling behavior is removed. See [Apple's native tiling shortcuts](https://support.apple.com/en-gb/guide/mac-help/mchl9674d0b0/26/mac/26).

Rest three fingers on the trackpad to show the overview without changing focus. Swipe up or down to select windows, or lift without swiping to dismiss. During a three-finger swipe, the overview stays open and focus follows your movement live. An ordinary swipe moves exactly one window, even when fast. To traverse more windows, keep your fingers down and continue moving after the overview appears; only then does speed-dependent acceleration engage. Lift after a fast sustained scroll for a stronger coast whose speed and distance follow your swipe velocity. Simply holding still does not unlock acceleration. A short swipe selects one window and holds the overview for 650 ms so the change stays visible. Rapid separate swipes keep that same overview open, advance one window per swipe, and restart the dismissal delay after the final swipe. Once sustained scrolling is active, holding near an edge continues through longer lists and gradually speeds up. A fourth contact immediately yields to native Space/Mission Control gestures, cancels Whalu navigation without restoring focus, and pauses window changes until the transition settles. Gestures and mouse-wheel navigation stop at the list ends; keyboard navigation wraps. New input catches momentum immediately. Hover over the visible thumbnails, hold the configured `overlay_modifiers` (Control + Option by default), or keep a Command + Tab/grave navigation session open to use the mouse wheel; scrolling is consumed while navigating so the active app does not scroll too. Mouse-wheel navigation preserves macOS scrolling distance and acceleration, including fractional movement from high-resolution and free-spinning wheels. Fast spins traverse immediately, slowing with the wheel’s own trailing input; there is no initial one-window gate or reduced traversal speed. Selection changes use the existing smooth thumbnail animation, with a restrained final coast of at most one extra window.

While any Windows overview is visible, the mouse’s Back and Forward side buttons select the next and previous window, respectively. This works for modifier holds, Command shortcuts, edge hover, three-finger contact/swipes, wheel navigation, clicks and brief selection feedback, with the pointer anywhere on screen. Each press switches once; repeated presses extend the 650 ms feedback. The buttons keep their normal actions when the overview is hidden, dismissing or suspended for a Space transition. A dedicated early button-only input tap intercepts visible-overview presses before mouse remappers; scrolling and other buttons keep their existing paths.

Hover at the left edge of any screen to open its Windows overview without changing focus. It stays open while the pointer rests at the edge or anywhere inside the visible thumbnail strip, including when keyboard modifiers are released or navigation feedback ends. Scroll the mouse wheel or use two-finger scrolling while hovering the thumbnails to navigate without holding modifiers. Leaving both areas immediately starts the slide-out and ends hover-owned wheel motion; keyboard holds and thumbnail drags keep their existing behavior. Dragging an ordinary window to the edge does not open the overview.

Holding the mouse button in another app temporarily hides the overview and suspends window discovery and focus changes. Releasing it refreshes the window list after 90 ms. This keeps synchronous Accessibility queries out of native window dragging; dragging thumbnails inside the overview still reorders the stack.

## Install and run

Install Whalu with Homebrew, then open it from Applications.

```sh
brew install --cask danileph/whalu/whalu
```

You can also download the [latest universal ZIP](https://github.com/danileph/whalu/releases/latest/download/Whalu-macOS-universal.zip), open it, and move Whalu to Applications. Whalu supports Apple silicon and Intel Macs on macOS 15 or later. Quit Whalu before replacing an existing installation.

Release builds are ad-hoc signed, without Apple notarization. If macOS blocks the app, review [Apple’s instructions for opening an unnotarized app](https://support.apple.com/102445). The release includes a SHA-256 checksum. Whalu guides you through its permissions after opening; updates can require removing and re-adding its entries.

To build from source, install Rust and Apple’s Command Line Tools, clone this repository, and run:

```sh
./scripts/install.sh
```

Requires macOS 15.0 or later, Rust, and Apple's Command Line Tools. The script builds an ad-hoc signed bundle, installs `~/Applications/Whalu.app`, and opens it. It does not install a login item. Quit the former Magnetic app before installing Whalu. Whalu has its own bundle identity (`dev.danileph.whalu`), so grant Accessibility and window-preview permissions to **Whalu** when prompted.

On first launch, Whalu retains a setup window and **W •** menu until permissions are ready. Enable the installed app in **System Settings → Privacy & Security → Accessibility**. If keyboard access is separately blocked, setup offers Input Monitoring settings. It starts automatically after permission is granted.

The overview is a vertical 3D wheel along the left edge of the display where it is opened. The selected thumbnail stays at the display's vertical center, full size and facing forward, with a blue outline. Neighboring cards gradually shrink, recede, and tilt vertically toward the edges; both ends turn right around the Y axis. The entire overview fades in from zero opacity while sliding 24 logical points right into its resting position, then fades out with the same short slide left. Both directions take 180 ms, keeping thumbnail spacing, size and perspective intact and releasing click and scroll ownership immediately on dismissal. Reopening mid-dismissal continues from the current on-screen positions. Selection changes glide smoothly in 180 ms, including rapid reversal; opaque edge cards travel out of view without fading. First and last entries stay centered with empty space on the unused side. Windows outside the visible thumbnail set appear as smaller, 16-point app icon badges with the same styling as thumbnail badges at the top or bottom of the strip, with one badge per window. Each hidden-window stack starts in line with the nearest thumbnail badge and extends right. The leftmost badge is the next hidden window beyond the visible thumbnails; subsequent badges continue that sequence to the right at both ends of the strip. Click an icon to select that window. Longer stacks wrap into compact rows; the thumbnail area reserves room for them so the selected card remains centered. App icon badges stay in front of the preview borders and follow each card's perspective; individual shadows remain subtle. Cards float over a transparent background with no visible captions. The system Reduce Motion preference disables rotation and animated travel. Grouping remains disabled.

Drag a thumbnail or app badge up or down to change its position in the stack whenever the overview is visible, including after a swipe or a quick Command shortcut. The card follows the pointer while neighboring cards make room. Hold near the top or bottom to scroll through longer stacks. Release inside the overview to drop; release outside to cancel. The overview stays open until the drag ends, even after releasing the shortcut modifiers. Reordering preserves the focused window and all native window positions. A click with small mouse movement still selects the window on release. Clicking a distant thumbnail selects it immediately when every window along the traversed path belongs to a different application. Those windows are then raised quietly underneath the selection, with native focus and real window order checked between steps. Paths with repeated application ownership, including separate processes with the same known app identity, use the existing step-by-step traversal in either direction. Compositor updates are allowed to settle before each step. If a background repair fails, the selected window keeps focus and the repair stops; its remaining background order may be incomplete. A new click retargets from the current window; keyboard or swipe input interrupts the remaining steps. When a Space has no focused window yet, its remembered window (or the first stack entry) supplies the starting point for background ordering or ordinary traversal.

The wheel fills the display's usable height, with clearance for the menu bar, Dock, shadows, and app badges. Taller displays show more neighbors; card size and minimum spacing stay consistent in macOS logical points, including on Retina displays. Connecting, disconnecting, rearranging, or changing the resolution of a monitor refreshes its layout and click targets without treating the change as a Space switch.

The monitor under the pointer when opening the overview shows windows visible in its current Space, even while another monitor's window has focus. Opening the overview leaves native focus unchanged; keyboard, wheel, trackpad, or thumbnail selection then navigates that monitor's list. Its monitor stays fixed through refreshes and the current navigation session; closing and reopening selects the pointer's current monitor. Each monitor keeps its own window order and remembered selection across Spaces. An empty monitor stays empty even when another monitor has visible windows. Stack edits, Space transfers, and tiling commands act on the actual focused window.

Window thumbnails require **Screen & System Audio Recording → Whalu** in Privacy & Security. Choose **W → Enable window previews…** to request access. Whalu takes small, individual-window snapshots on demand while the overview is open; it does not record audio, start a video stream, upload captures, or save them to disk. Without access, the same cards show app icons. Only the visible card set is cached in memory.

The **W** menu provides refresh, preview access, configuration, reload, and quit. This development bundle's ad-hoc signature changes when rebuilt. If setup reports denied while Settings shows Whalu enabled, remove that stale entry and add the current installed bundle again. The running GUI app's permission result is authoritative; terminal diagnostics can inherit the terminal's access.

## Configuration and recovery

Default location: `~/Library/Application Support/Whalu/config.toml`. On the first default launch, Whalu copies `config.toml` and `state.json` from the former `Magnetic` support directory when the corresponding Whalu file is absent. Existing Whalu files take precedence; the originals are retained. Legacy Magnetic/Floating metadata is retained for compatibility and has no effect on geometry.

Choose **W → Open configuration**, edit, then **W → Reload configuration**. Errors leave the previous valid configuration active. `--config /absolute/path/config.toml` selects another file.

```toml
overlay = true
overlay_delay_ms = 120
gestures = true
ignored_apps = ["com.apple.QuickTimePlayerX"]
```

Legacy `overlay_brightness`, `gap`, and `magnetic` values are accepted and ignored; generated configurations omit them.

Set `gestures = false` for overview and modifier-wheel operation without three-finger navigation.

Bindings use physical macOS keycodes and exact modifier combinations. A supplied `bindings` array replaces the defaults. To add Command + Esc to a custom array, bind action `recent` to keycode `53` with `command = true`. Removed actions (`group_next`, `group_previous`, and the former group-local `left`, `down`, `up`, `right`) must be removed from older custom configurations. Generate and check the current schema with:

```sh
~/.cargo/bin/cargo run -- --print-config > whalu.example.toml
~/.cargo/bin/cargo run -- --check-config whalu.example.toml
```

Window modes are atomically saved in `state.json` beside the default configuration. Recovery matches unique, nonempty app/title identities and never moves or resizes windows at startup. Ambiguous windows start Magnetic. Old grouping metadata is ignored and no longer written. Previous-size history belongs to macOS; Whalu stores no restore rectangles. Stack order is retained across visibility changes and Space switches during the current session.

After switching native Spaces without a focused window, the first navigation command enters the remembered visible window, or the first/last available entry. Commands during the native transition wait for it to settle. Opening the overview still refreshes the destination immediately; the navigation pause does not postpone its window list. Before drawing, the overview checks the display's current native Space and rejects a stale stack, including when workspace notifications arrive late.

## Platform boundaries

- Public APIs do not provide stable native Space IDs. The adapter uses AX/Quartz window identity and overlapping visibility cohorts tracked separately for each display. Sticky windows, identical windows across Spaces, and wholesale window replacement can confuse context identity. Space transfers use the native managed-Space bridge to move the focused window, then ask Dock to slide to its destination on the same display. The cursor stays untouched, and modifiers can remain held throughout repeated moves. The overview stays open and replaces its list with the destination's windows after each completed transfer. Old thumbnail targets are disabled during the switch. Transfers stop at display boundaries and fullscreen neighbors; sticky windows are left alone. Dock draws a brief native transition with travel bounded to the adjacent desktop and no release momentum. The destination list is staged during the slide; previews are prefetched and retained across Spaces in a cache of up to 48 images. Whalu verifies final membership, visibility, and focus after the transition. No mouse dragging or window geometry changes are synthesized. The gesture encoding supports macOS 15–26; unsupported versions fail before moving a window. These private macOS operations may change in future releases.
- Window capture IDs use the optional Accessibility `_AXUIElementGetWindow` SPI so windows with identical titles and bounds (including Finder windows) retain their own thumbnails. If that symbol is unavailable, discovery falls back to title/geometry matching and independent icon cards for fully visible ambiguous windows.
- Three-finger detection uses the experimental, read-only private MultitouchSupport framework. The public CGEvent tap consumes the three-contact scroll stream, including its trailing native momentum, so it cannot also scroll the active app. Ordinary two-finger and native four-finger input remain available, and no trackpad settings are changed. Keep system Space/Mission Control gestures on four fingers and three-finger dragging disabled. Keyboard navigation remains available if the listener is unavailable. ABI reference: [Hammerspoon touchdevice header](https://github.com/asmagill/hs._asm.undocumented.touchdevice/blob/master/MultitouchSupport.h).
- Dialogs, minimized/fullscreen windows, and windows without settable AX positions are excluded from active navigation. Focused modal/sheet/fullscreen UI suspends Whalu commands. Native tiling availability and minimum-size behavior belong to macOS and the target app.
- Hardware validation covers this Mac, not every app, monitor configuration, or macOS release.

## Empty Spaces immediately switch back

Check `defaults read com.apple.finder CreateDesktop` if entering an empty Space immediately returns to a window on another Space, including when Whalu is stopped. Setting this preference to `false` disables Finder's desktop surface, rather than only hiding its icons. On the tested macOS 26.2 system, WindowServer restored an application whose window belonged to another Space; that application's window ordering then triggered the return. Restoring the desktop surface resolved the behavior with Whalu running:

```sh
defaults write com.apple.finder CreateDesktop -bool true
killall Finder
```

To keep desktop icons hidden, use **System Settings → Desktop & Dock → Show Items → On Desktop** instead. This keeps the desktop surface available. The **When switching to an application, switch to a Space with open windows for the application** preference can remain enabled.

## Brand assets

The [brand assets](assets/README.md) include the glass app icon, silver wordmark, generation prompts, and icon rebuild instructions.

## Landing page

The React landing page lives in [website/](website/README.md), with a recorded product demo in a MacBook frame and a GitHub Pages deployment workflow. Run `npm ci` and `npm run dev` from that directory to preview it locally. The configured site URL is [danileph.github.io/whalu](https://danileph.github.io/whalu/). GitHub Pages publishes changes from `main`; see the [deployment workflow](https://github.com/danileph/whalu/actions/workflows/pages.yml) for its current status and the website README for details.

## Development and releases

Use `develop` for ongoing work. It is the repository’s default branch; `main` holds published changes. CI checks the website, Rust formatting, tests, Clippy, and the macOS build on both branches and their pull requests. GitHub Pages deploys only from `main`.

To publish a stable app release, update the version in `Cargo.toml` and `assets/Info.plist` on `develop`, increment `CFBundleVersion`, run the required checks, and merge the changes into `main`. Push `main`, then create and push an annotated `vX.Y.Z` tag at that commit. `.github/workflows/release.yml` rejects tags outside `main` or with mismatched versions, builds both Mac architectures, verifies the signed universal archive, and publishes the ZIP and checksum to GitHub Releases. The landing page’s `/releases/latest/download/Whalu-macOS-universal.zip` link follows new stable releases automatically.

After the GitHub release is live, update the Homebrew cask using the instructions in the [Whalu tap repository](https://github.com/danileph/homebrew-whalu). The updater verifies the published ZIP against its checksum and pins the cask to that release.

For a local release archive, install both Rust targets with `~/.cargo/bin/rustup target add aarch64-apple-darwin x86_64-apple-darwin`, then run `RELEASE_TAG=vX.Y.Z ./scripts/package-release.sh` with the current version. Archives are written to `dist/`. The ordinary `./scripts/build-app.sh` remains a build for the current Mac; `--universal` builds both architectures.

These builds use ad-hoc signing. Developer ID signing and Apple notarization are not configured; adding them requires the maintainer’s Apple credentials and a separate signing setup.

## Verification

```sh
~/.cargo/bin/cargo test
~/.cargo/bin/cargo clippy --all-targets -- -D warnings
~/.cargo/bin/cargo fmt --check
~/.cargo/bin/cargo run -- --native-smoke-test
~/.cargo/bin/cargo run -- --input-smoke-test
~/.cargo/bin/cargo run --release -- --performance-probe
~/.cargo/bin/cargo run -- --gesture-probe 10
~/.cargo/bin/cargo run -- --preview "$PWD/tmp/hud-preview.png"
```

Append `--hold` to `--preview` to watch a 25-second native wheel demonstration, including selection changes, reversal, and list ends. The preview also checks live compositor interpolation, interrupted reversal, stable card identity, selection centering, and 3D transform retention.

Tests cover independent window/app navigation, continuous swipe reversal and isolation, unfocused Spaces, legacy-state migration, stack reordering without focus/geometry changes, all screenshot shortcut aliases, modifier-first key release, native-shortcut passthrough, Command + Tab/grave session ownership and wheel routing, and configuration validation. The optional native smoke check manipulates only temporary child-process fixture windows and requires Accessibility. It verifies enumeration, real focus, continuous selection, unchanged selection geometry, fixture-only Space transfer and return when two regular desktops exist, keyboard-tap creation, and a non-key HUD.

Scroll tests also cover natural direction, early three-contact ownership, partial release, native momentum isolation, modifier-wheel routing, bounded deceleration, reversal, and fresh-input interruption. Native fixture checks exercise short-swipe selection, kinetic focus, wheel bursts, and the 650 ms feedback/dismissal lifecycle without writing recovery state. The focused `--input-smoke-test` runs the keyboard, wheel, gesture, and click regressions on fresh single-app, separate-app, and mixed-app fixture cohorts, independently of native tiling and Space transfers. Quit the running manager before native smoke checks to isolate fixture focus and input.

The runtime coalesces AX/workspace/display notifications on the main run loop and uses one-shot timers for bursts and transitions. No periodic window enumeration runs at idle. Input callbacks enqueue work without performing Accessibility calls.

Keyboard navigation reuses discovered windows after checking fresh Quartz visibility/geometry and the focused AX window. Window changes, dialogs, Space/display transitions, configuration reloads, and uncertain focus fall back to full discovery; stack edits and Space transfers always refresh metadata. Overview icons are cached per application process. Recovery snapshots are coalesced on a background writer, which flushes the latest state on quit. `--performance-probe` measures full discovery and validated cache checks without changing focus or moving windows; keep the desktop steady while it runs. These timings exclude native focus activation and compositor animation.
