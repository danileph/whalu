# Overview stranded on another desktop

Investigated on 2026-10-28, macOS 26.2, against the still-running installed build 43. The app had been open for more than two days. It was not restarted or refreshed before capturing the failure.

## Captured failure

During the user's ten-second Control + Option hold on Desktop 2 (native Space 6):

- The modifier observer saw both keys held; Whalu's input tap was enabled and secure input was off.
- Whalu had five overview cards. Its panel's AppKit `isVisible` value was true, but `isOnActiveSpace` was false.
- WindowServer assigned that same panel only to Space 5 (Desktop 1), and excluded it from the on-screen window list on Space 6. Its frame fit the current display.
- AppKit's collection behavior was still 337: `CanJoinAllSpaces`, `Stationary`, `IgnoresCycle`, and `FullScreenAuxiliary`. The native sticky tag was also still present. Reading that policy alone would falsely suggest that the window could appear everywhere.
- The same panel appeared on returning to Desktop 1.

These observations establish a native Space-association failure, rather than missing cards, missing preview permission, or an unreceived shortcut. The display setup had changed and the machine had slept since launch, but that history alone does not identify a trigger.

## Reproduction and recurrence

A fresh fixture reproduced the captured state on its first fullscreen cycle:

1. Open and hide a normal overview panel.
2. Enter native fullscreen with a disposable fixture window, creating a fullscreen Space.
3. Exit fullscreen, deleting that Space.
4. Move the fixture to the neighbouring regular desktop and open the same overview panel.

The panel remained assigned to the former desktop while AppKit reported `isVisible=true`, `isOnActiveSpace=false`, and collection behavior 337. This provides a reproducible lifecycle trigger without requiring a long uptime or changing physical monitors.

Whalu configured all-Spaces behavior once, when creating its long-lived panel. It then relied on `orderFrontRegardless` and the saved collection policy on later openings. Fullscreen Space creation/deletion can invalidate the native assignment without updating that policy. A fresh launch starts with a healthy assignment, explaining why restarting or testing only ordinary Space transfers can appear to resolve the problem until the lifecycle repeats. The exact internal macOS operation that causes the divergence was not traced; the observable stale assignment and triggering sequence were established.

Reapplying the cached policy, temporarily toggling it, ordering with `MoveToActiveSpace`, and closing/reopening the same NSPanel did not repair the regression. Apple's [collection-behavior documentation](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct) describes the policy preferences; those flags alone were insufficient in this captured state.

## Repair and regression

Before ordering an overview whose native active-Space check fails, Whalu now explicitly binds that overview window to the current native Space of its target display. It uses the existing WindowServer bridge already used for Space transfers, without a Dock swipe, application activation, or a key-window request. An ownership check rejects any window whose Quartz owner PID is not Whalu's own process. Cards, cached images, focus, and user-window geometry remain intact. A diagnostic line records the old assignment and requested destination when this repair runs.

The native regression checks actual Quartz on-screen membership as well as AppKit active-Space status. It exercises four fullscreen creation/deletion cycles and neighbouring-desktop returns, checks retained native fixture focus and geometry, and verifies that the rebinding helper rejects a foreign fixture window. This covers repeated lifecycle invalidation, rather than merely demonstrating one successful opening after a restart.


A control build with only the new rebinding block removed, retaining the same strengthened visibility assertions, again failed on cycle 0 with the original `visible=true, active=false, onscreen=false, membership=[former desktop]` state. The release build with rebinding enabled passed all four cycles. Thus the comparison does not rely on a fresh launch merely happening to work.

The final installed build 44 passed the same four-cycle regression with real captured previews, all five native transfer animations, and preserved focus/geometry. Its Space fixture has an independent temporary app identity, preventing controller/child activation grouping from short-circuiting native swipes. Version 0.5.1 (build 44) is installed and running with refreshed Accessibility, Input Monitoring, and Screen Recording grants. Raw local diagnostic evidence is retained in `~/Library/Logs/Whalu/investigations/20261028/`.
