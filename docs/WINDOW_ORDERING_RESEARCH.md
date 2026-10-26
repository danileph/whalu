# Immediate selection with background window ordering

Investigated on 2026-10-25, macOS 26.2 (25C56), with System Integrity Protection enabled.

## Finding

An immediate destination focus followed by background ordering is feasible in the tested fixtures when all intervening windows belong to other application processes. It is not a general replacement for Whalu's current traversal: raising another window of the selected application covered the destination and changed native focus.

A conditional fast path is worth considering. A universal implementation that promises to keep the destination visible and focused has not been established.

## Method

A standalone Objective-C diagnostic created five disposable AppKit windows. Three layouts were exercised:

- Five independent application processes.
- Five windows in one application process.
- Four processes, with windows 2 and 3 sharing a process and window 5 belonging to a separate process.

The controller validated each fixture window's Quartz ID and owning process. Each scenario started with native AX focus on window 5, its application in front, and fixture order `5, 1, 2, 3, 4`. Invalid focus setup aborted the test. Only fixture windows were reordered. The diagnostic closed its children and restored the previously active application afterward.

After each Accessibility raise, Quartz order and actual AX focused-window identity were sampled for approximately 80 ms. This detects sustained coverage/focus loss; discrete samples do not prove absence of every possible single-frame artifact. The fixtures were responsive native AppKit windows, not a compatibility survey of third-party apps.

The installed Whalu bundle and its permissions were unchanged. Its normal GUI process remained running after the investigation.

## Accessibility results

The controller performed `AXRaise` without setting AXMain, AXFocused, AXFocusedWindow, or activating the intermediate applications. These are deliberately different operations from Whalu's normal `Effect::Focus` path.

| Fixture layout | Background raises | Final front-to-back order | Native focus | Samples where 5 lost focus or front position |
| --- | --- | --- | --- | --- |
| Five separate processes | 2, 3, 4 | 5, 4, 3, 2, 1 | 5 | 0 of 131 |
| Windows 2 and 3 share a background process | 2, 3, 4 | 5, 4, 3, 2, 1 | 5 | 0 of 96 |
| All windows share the selected process | 2, 3, 4 | 4, 3, 2, 5, 1 | 4 | 175 of 175 |

The same-application problem appeared at the first raise, not just at completion. Raising window 4 while window 5 was selected produced order `4, 5, 1, 2, 3` and native focus 4.

The requested background sequence `4, 3, 2` also needs an ordering correction. With separate processes it kept focus on 5 but finished as `5, 2, 3, 4, 1`: each successive raise inserted a window immediately behind the active application's front window. For the desired final order `5, 4, 3, 2, 1`, background raises must run bottom to top: `2, 3, 4`.

## Direct relative-ordering attempts

Apple's [`NSWindow.orderWindow(_:relativeTo:)`](https://developer.apple.com/documentation/appkit/nswindow/order%28_%3Arelativeto%3A%29?language=objc) supports ordering a receiving window above or below another window. An owning fixture successfully placed its own window 4 immediately below window 5 while preserving focus on 5. Whalu does not own other applications' NSWindow objects, so this is not directly available as a generic controller operation.

The controller also tested private APIs discovered in the local SkyLight runtime and compared with [yabai's declarations](https://github.com/asmvik/yabai/blob/master/src/misc/extern.h) and [ordering implementation](https://github.com/asmvik/yabai/blob/master/src/osax/payload.m):

- `SLSOrderWindow(mainConnection, foreignWindow, -1, anchor)` returned 1000 and left ordering unchanged.
- `SLSTransactionOrderWindow` followed by a transaction commit left the foreign windows unchanged. Private transaction return values were not used as proof of success; the observed compositor order was the check.
- Constructing an `SLSWMBridgedWindow` for a foreign fixture ID and requesting relative ordering also left the compositor order unchanged.

These tests do not exhaust every private bridge or privileged approach. They do establish that the tested direct calls are not a working solution in the current process architecture. Yabai routes arbitrary ordering through its scripting addition; its [SIP documentation](https://github.com/asmvik/yabai/wiki/Disabling-System-Integrity-Protection) explains that this addition uses Dock injection and requires partially disabled SIP. No injection, system-protection changes, or permission changes were attempted here.

## Recommendation

A future prototype could:

1. Validate the current Space, monitor, window IDs, and application ownership.
2. Focus the clicked destination immediately through Whalu's existing focus path.
3. When every intervening window has a different PID from the destination, raise those windows quietly, bottom to top, without intermediate application activation.
4. Verify both final Quartz order and destination focus. Cancel pending work immediately on a new selection, external focus change, window disappearance, or Space/display transition.
5. Keep the current sequential traversal for same-application cases and other unsupported situations.

The conditional path needs real-app compatibility tests and cancellation/failure handling before adoption. Accessibility response timing and application-specific window grouping remain unverified outside these fixtures. Repeatedly re-raising the destination after same-app raises would restore the final state, but the observed intermediate focus and coverage changes mean it would not establish the proposed uninterrupted experience.

No application behavior was changed as part of this investigation.


## Implementation (2026-10-26)

Build 40 enables the conditional path. It selects the destination immediately, then performs quiet AX raises in traversal order underneath it. The current monitor, native Space, overview epoch, window/process/Quartz identities, actual native focus, and observed compositor order are checked between steps. Each raise must become visible in the ordering before another is issued; an unresponsive ordering change has a 250 ms deadline. New navigation cancels pending work. Failed repairs replay the original traversal only while the destination still owns native focus; external focus is respected. Repeated application ownership anywhere along the path (including separate processes with the same known bundle identity) and missing native identities retain ordinary traversal. Installed-GUI checks showed that shared background applications can fail the relative-order check, so the production eligibility rule is stricter than the initial feasibility fixtures.

The native input suite now covers single-app, separate-app, and mixed-app fixtures, including immediate target selection, stable destination focus, final order, unchanged geometry, missing/stale starting focus, retargeting, keyboard/swipe interruption, stale identities/overviews/Spaces, and fallback after an ordering timeout.


## Focus retention after a failed repair (2026-10-26)

The user's build 40 recording shows Telegram → Terminal → Telegram around 2.8 seconds and again around 5.2 seconds. This matches the foreground fallback used after the fast path had already selected Telegram. AX focus can settle before Quartz reports the destination at the front; build 40 treated that initial mismatch as an immediate failure instead of waiting for compositor settlement.

Build 41 applies the initial 600 ms settlement deadline to both destination focus and its observed ordering. Subsequent quiet raises retain their 250 ms deadlines. Once the destination is selected, its foreground traversal path is consumed: a failed AX raise or ordering timeout stops background repair and schedules a census while preserving destination focus. There is no foreground replay. A partial repair may leave intervening windows incompletely ordered. Paths rejected before immediate selection still use ordinary sequential traversal.
