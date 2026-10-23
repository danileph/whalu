# Validation record

Test machine: Apple silicon MacBook Air, Darwin 25.2.0 (macOS 26 family). This record describes the initial implementation, not universal app or OS compatibility.

- Rust acceptance suite: 18 scenarios pass.
- `cargo clippy --all-targets -- -D warnings`: passes.
- Native Accessibility enumeration: fixture windows correlated to Quartz-visible windows without requesting Screen Recording or using private window-ID functions.
- Native position-only update: exact requested X/Y observed; original width/height preserved.
- Native application activation and target-window focus/raise: verified through AXFocusedWindow and the workspace frontmost application.
- Native Floating snap: left-half usable-display rectangle accepted.
- Native snap restore: original pre-snap rectangle recovered.
- Global keyboard event tap: creation succeeds.
- Runtime: menu-bar process starts with arrangement/overlay disabled for a read-only smoke session; sampled idle CPU was 0.0%; test process stopped cleanly.
- Native HUD: renders via AppKit; does not become the key window; PNG reviewed.

A real native detail surfaced during testing: AppKit can constrain a resize at the window's old position. The adapter resizes, moves, then reapplies the target size and reads the actual rectangle. Failed/constrained snaps are removed from their proposed group.

Still requiring hands-on product validation: extended real-app focus and typing sessions, actual keyboard/trackpad conflict behavior across system settings, Space animations and Mission Control, separate display Spaces, monitor disconnection, latency profiling, and three-finger gesture feasibility. Stable native Space IDs remain unavailable; gesture handling is covered by the later update below.

## Startup regression fix

The original Finder launch exited after an Accessibility failure, leaving no menu icon. The installed app now retains a setup window and M • status item, opens the correct permission pane, and retries permission/event-tap setup automatically. The setup window and Open Accessibility Settings button were verified through the actual installed app UI.

Settings can retain an enabled permission for an older ad-hoc signature. The GUI app’s own permission result is authoritative; CLI checks inherited from a trusted terminal are insufficient to validate Finder launch. After the current installed copy was re-added and enabled, the same process automatically entered its running state. Its live HUD was observed listing the real windows, and the user confirmed Control + Option + Up/Down navigation works. No further rebuild was performed after this grant.

## Space, group, and three-finger update (0.1.1)

- Regression suite: 29 core acceptance tests plus 7 gesture tests pass; formatting and strict Clippy checks pass. Native fixture enumeration, focus, move, snap, restore, keyboard tap, and HUD checks also pass.
- Installed 0.1.1 reacquired Accessibility access and reports one active trackpad listener.
- Navigation can enter a context with no focused window, using its last visible focus or first/last eligible entry. Geometry commands still require a real focused window.
- Canonical groups are separate from visibility and survive partial, empty, and changed cohorts. Hidden members are never navigation targets. Recovery captures canonical membership while its Space is inactive.
- Only observed direct mouse dragging/resizing can detach a group member automatically. Space/app frame changes are reconciled without ungrouping.
- Native discovery retains windows omitted by off-Space AX snapshots, using public Quartz IDs where unambiguous. A stale focused window from the previous Space does not freeze the destination’s navigator.
- Commands received during Space transitions are queued until settling.
- The experimental MultitouchSupport listener loaded this Mac’s one built-in trackpad. With the user performing physical swipes, the 120-second hardware probe recorded 2,316 contact frames, 16 recognized three-finger swipes, and zero rejected ABI frames.
- Three-finger recognition has synthetic regression coverage for every direction and two/four-finger isolation. End-to-end physical swipe/navigation validation is recorded after installation.

## Continuous live navigation (0.1.2)

- 44 tests pass: 29 core scenarios and 15 continuous gesture/navigation scenarios. Strict Clippy and formatting checks pass.
- Three-contact frames now emit Begin, signed selection updates, and End/Cancel. Selection occurs before release; movement can traverse several candidates and reverse in the same session. Holding near an edge continues through longer lists. Two/four-finger gestures remain excluded.
- Candidate order is fixed for each session. Vertical navigation visits groups and their remembered member; horizontal navigation stays within the starting app. List ends clamp, and reversing after overshooting changes selection without requiring an equally large return movement.
- The non-key overview stays visible until the last finger lifts. Focus changes live, while automatic positioning is suppressed until the session finishes. Native Space changes abandon the old session; cancellation restores original focus only if it remains visible in the same context.
- The release binary passed native fixture enumeration, move, snap, restore, event-tap creation, and HUD checks. A new native check repeatedly selected and reversed between both fixture windows through real AX focus with the gesture overview visible, confirmed that the panel never became key, and read back unchanged native rectangles.
- Installed bundle signature verified. The installed app resumed after its existing Accessibility entry was refreshed and reports one active trackpad listener. The user then confirmed on the physical trackpad that the overview stays open and focus follows the gesture, including reversal before release.

## Existing half-split grouping (0.1.3)

The user reported that adjacent half-width Codex and Telegram windows were separate group entries. Live readback confirmed 624×787 windows at (24,37) and (656,37), while saved membership for both was null. Earlier grouping required Magnetic's in-memory snap metadata, so an existing native/manual split was missed.

- Reconciliation now recognizes an unambiguous complementary left/right half arrangement from actual current geometry, including native zero-inset halves and small position drift from earlier automatic exposure. It updates group metadata without moving/resizing either member.
- Recognition requires one candidate per half on the same visible display. It leaves overlapping/ambiguous candidates, non-half rectangles, and durable groups with visible or hidden members alone. It is suspended during drags, modal UI, Space transitions, and active scrubbing.
- 49 tests pass (34 acceptance + 15 gesture scenarios), including the actual Codex/Telegram geometry, keyboard and swipe group skipping, native halves, ambiguity, hidden members, frozen transitions, and manual detachment. Strict Clippy and formatting checks pass.
- Release native smoke check passed: two real fixture windows were placed into halves, their snap/group metadata was cleared, recognition formed one group, group navigation did not select the other member, and readback confirmed unchanged rectangles. Existing focus/gesture/HUD/snap/restore checks also passed.
- Version 0.1.3 installed and signature verified. After the user authenticated the permission refresh, the installed app resumed with one trackpad listener. Its saved live state assigns both Codex and Telegram to group 9. Native readback confirms their rectangles remain (24,37) 624×787 and (656,37) 624×787. No resnap or geometry edit was required.

## Exact snap fractions and gap removal (0.1.4)

- Root cause: snap regions previously applied the automatic-layout gap as an inset on every edge. With gap=8, each half was 16 points narrower than half the usable display, leaving 16 points between adjacent windows.
- Snap geometry now uses exact full/half/quarter fractions with no inset. The gap setting continues to control background exposure only. Tests verify shared edges, full area coverage, odd dimensions, negative display origins, and independence from automatic-layout spacing.
- 50 tests pass (35 acceptance + 15 gesture tests); strict Clippy and formatting checks pass. Release native smoke checks pass with the new exact snap regions.
- Added explicit --fit-splits action for refitting existing unambiguous half splits; it reads back actual native rectangles and restores the original pair if a window rejects exact sizing. It does not run as part of automatic layout.
- Applied the explicit refit to Codex and Telegram: native readback reports (0,29) 640×803 and (640,29) 640×803 on the 1280-point usable display. Their adjacent edges meet at x=640.
- Installed 0.1.4, verified signature, refreshed existing Accessibility identity, and verified normal runtime plus one active trackpad listener. Both windows retain group 9 and their exact rectangles after startup.

## Minimum-size snap anchoring (0.1.5)

- The reported window enforced a minimum width of 720 points. A right-half snap on this display previously placed that width at x=640, extending 80 points past the usable right edge at x=1280.
- After resize/move/resize, snapping now reads the app-accepted size, aligns the requested right/bottom outer edges, and clamps its position to the usable display without resizing again. It corrects position even when the resize is rejected. Oversized windows that cannot fit preserve their minimum dimensions and keep their top-left reachable.
- 51 tests pass (36 acceptance + 15 gesture). Regression coverage includes right/left half and quarter anchoring, width/height constraints, negative display origins, oversized windows, and unchanged unconstrained snaps. Strict Clippy and formatting checks pass.
- Release native smoke test passes. A separate child fixture enforces minimum dimensions of 70% display width and 60% display height; right-half, bottom-right, and top-left snaps were read back and confirmed inside the usable display, with right/bottom edges preserved as applicable.
- Installed 0.1.5 signature verified; existing Accessibility identity refreshed. Installed runtime reports normal operation and one active trackpad listener. The user confirmed that Simplenote stays fully visible with its right edge aligned after the right-snap shortcut.

## Foreground groups and actual group presentation (0.1.6–0.1.7)

The repeated grouping report exposed several distinct problems. A covered Terminal half was counted as a competing half candidate, Telegram retained its older Terminal partnership, and group entry only raised one member. The user's initial positive response to 0.1.6 was corrected: the pair was not skipped and overview changes did not match the visible result. It was not treated as final validation.

- Discovery now obtains public front-to-back on-screen window order separately from the all-window list used for identity retention. Rectangle-union subtraction identifies substantially exposed windows (at least 80%) for new group recognition. Covered windows remain in the normal navigation stack. Unknown layer-zero windows still count as occluders.
- A focused, clearly presented half pair can replace an old partner when all affected members remain on the same active Space. Groups containing off-Space members are preserved. Changing background presentation alone does not replace partners.
- Manager-driven focus is tracked separately from external presentation intent. Its own navigation cannot cause recognition to regroup a half window left behind with the newly focused window. External focus changes, direct mouse interaction, explicit snaps, and Space changes permit reconciliation again.
- Keyboard group jumps and vertical scrubs bring every visible destination-group member forward and focus the selected member last. Focus requests explicitly set AXFocusedWindow/AXFocused in addition to application activation and AXRaise.
- 58 tests pass (43 acceptance + 15 gesture). Strict Clippy and formatting checks pass. New scenarios cover occluded half duplicates, unknown overlapping occluders, full-window occlusion, replacing an old same-Space partner, preserving inactive groups, navigation intent isolation, and group presentation order.
- Final release native smoke passed with three real child-process windows: an exposed pair, a covered competing half, and an old partner relationship. Recognition selected the foreground pair. After deliberately covering a member, group presentation restored both halves and verified the selected member was actually focused. Existing geometry, fixed-size, constrained snap, gesture-selection, event-tap and HUD checks also passed.
- Installed 0.1.7 signature verified. Existing Accessibility identity refreshed after user Touch ID. Runtime reports one trackpad listener. The user then confirmed that actual windows switch correctly together and the screen follows the overview.

## Overview group rendering (0.1.8)

The user reported that the overview appeared to disconnect a pair while the actual windows remained connected. Live state assigned Codex and Terminal to the same canonical group (19), with Telegram between their entries in the global stack. The HUD rendered that stack directly and emitted the same group heading again after the intervening window, visually splitting one group into multiple sections.

- Overview display order now keeps all visible members of each group contiguous under a single heading. Canonical membership, individual-window stack order, gesture targets, and native geometry are unchanged.
- Each row displays its original stack number so individual-window navigation retains a clear reference even when group presentation gathers nonadjacent members.
- 59 tests pass (44 acceptance + 15 gesture). The new regression reproduces nonadjacent grouped windows, verifies one heading block, and checks that window navigation and membership do not change. Strict Clippy and formatting checks pass.
- Native HUD preview uses nonadjacent Codex/Terminal members with Telegram between them in the input stack. The rendered PNG was inspected: the pair appears together once under Group 1 and Telegram under Group 2, with stack numbers 1/3 and 2 respectively. No clipping was observed.
- Version 0.1.8 installed and signature verified. The OS authentication prompt was completed, the existing permission identity was refreshed, and the installed app resumed normal operation with one trackpad listener.

## Grouping deferred; individual-window model (0.1.9)

At the user's explicit request, grouping is removed from the current product. This section supersedes the earlier grouping behavior and its validation records.

- Removed group IDs/membership, recognition, occlusion analysis, presentation-intent tracking, multi-window group activation, group recovery, and the split-refitting CLI. The native adapter no longer makes an extra front-to-back window scan for grouping.
- Overview and vertical swipes use the same Space-local individual-window stack. Horizontal swipes still select the active app's windows. Live focus, uninterrupted traversal/reversal, and lift-to-finish remain.
- Removed the group-jump Shift shortcuts and group-local H/J/K/L shortcuts. They are no longer intercepted. Current example configuration contains only window/app navigation, mode toggle, and explicit snap/restore actions.
- Legacy saved group fields are ignored; new saved state contains only window identity and mode. A pre-removal source snapshot and state backup are retained under tmp/ for recovery.
- Explicit full/half/quarter snapping remains gap-free, with minimum-size edge anchoring and restore. Snapped window rectangles are independent and excluded from automatic repositioning until directly edited or resnapped.
- 36 tests pass (21 acceptance + 15 gesture); obsolete group-specific tests were retired. Current tests cover individual window/app navigation, every-window vertical scrubbing, Space history, legacy-state migration, removed shortcuts passing through, independent snap ownership, fixed-size layout, Floating, exact geometry, and minimum-size constraints. Strict Clippy and formatting checks pass.
- Native flat overview preview was inspected: one ordered list, no group headings, matching stack numbers, and no clipping.
- Initial native smoke attempt ran while the Mac was locked. After user unlock, a fixture-startup race was corrected by finishing AppKit launch before discovery and using a wall-clock startup allowance. Final release native checks passed: AX enumeration/focus, individual continuous selection/reversal, unchanged selection geometry, snap/restore, minimum-size constraints, event-tap creation, and non-key HUD rendering. The final 0.1.9 bundle is installed and signature-verified; its existing Accessibility identity was refreshed. Runtime is running with one trackpad listener and saved state contains no grouping fields.

## Stage Manager-style overview (0.2.0)

- Replaced the text panel with a transparent, non-key strip at the left edge of the focused window's display. Each individual window has a rounded card, app icon badge, truncated title, and blue selection outline. Long stacks keep the selected card in view with overflow counts; grouping remains disabled.
- At the user's request, reduced cards from 172×108 to 128×82 points, badges from 36 to 28 points, and the strip from 212 to 160 points. The final fallback render was visually inspected for spacing, title truncation, selection, and clipping.
- Added on-demand ScreenCaptureKit screenshots filtered to individual window IDs. No audio capture or persistent capture stream is requested. Only the visible thumbnail working set is retained in memory; removed/off-viewport entries are evicted. Pending requests and a refresh interval bound capture frequency. Missing permission/capture results retain icon/title cards. Capture callbacks retain immutable CGImages until main-thread NSImage conversion, and stale window-ID results are discarded.
- Screen capture access is requested only through the explicit menu action; regular navigation uses a permission preflight without prompting. The user approved adding Magnetic to Screen & System Audio Recording; Settings shows it enabled. Minimum supported macOS is now 14.2.
- 36 core/gesture tests pass. Strict Clippy and formatting checks pass. Final release native smoke passes AX enumeration, live focus/reversal with a non-key overview and unchanged selection geometry, fixed-size positioning, explicit snap/restore, constrained-size edge anchoring, and event-tap creation. Installed bundle signature verified; existing Accessibility identity refreshed.
- Initial user check found the smaller cards but no thumbnails. Restarted the installed app after the permission refresh; system logs now show individual-window ScreenCaptureKit screenshot requests. The user confirmed that actual window thumbnails now appear after the restart.

## Frosted overview without captions (0.2.1)

- Added a persistent native NSVisualEffectView using the HUD material, behind-window blending, and an explicitly active dark appearance. Rounded 24-point corners, a subtle border, and the panel shadow provide the frosted backdrop requested in the reference. The backdrop survives card refreshes while swiping.
- Removed all visible overview text: section headings, window/app titles, mode markers, counts, overflow captions, and control hints. Card labels remain available to accessibility tools without visible text. Empty Spaces hide the overview. Compact 128×82-point previews and 28-point icon badges are retained; vertical spacing is tightened after removing captions.
- Visually inspected the native sample render for caption removal, card/icon spacing, rounded backdrop, and selection outline. The bitmap cache cannot validate live behind-window blur; final installed appearance is checked separately.
- Strict Clippy and formatting checks pass. Release native smoke passes AX enumeration/focus, continuous live focus/reversal, non-key overview, fixed-size positioning, snap/restore, minimum-size constraints, and event-tap creation. No navigation or geometry logic changed.
- Installed 0.2.1 and verified its signature. Refreshed the existing Accessibility and screen capture entries for the new ad-hoc signature, then used macOS Quit & Reopen to activate capture access.

## Diffuse overview background (0.2.2)

The user clarified that the blur should merge into the desktop like the area behind Notification Center, rather than form a distinct rounded panel. This supersedes the enclosing backdrop introduced in 0.2.1.

- Removed the backdrop border, corner shape, and panel shadow. Extended native behind-window blur to the display's left edge and beyond the thumbnail strip, with a gradient mask fading toward the desktop and separate 45-point top/bottom fades. The native backdrop is a sibling of the cards, so its mask does not fade thumbnails or icons.
- Kept compact card/icon sizes, the selection outline, and the complete removal of visible captions. Background gradient updates disable implicit animation during navigation and asynchronous thumbnail refreshes.
- Inspected the final sample render and PNG alpha values: the center remains opaque, top/bottom edges approach zero alpha, and the right edge fades to transparency. This confirms feathering without an enclosing border; native live backdrop sampling is provided by the same visual-effect material validated in 0.2.1.
- Strict Clippy and formatting pass. Final release native smoke passes enumeration, focus/reversal, non-key overview, unchanged selection geometry, snap/restore, constrained sizing, and the event tap.
- Installed 0.2.2, refreshed its existing Accessibility and screen capture identities, and used macOS Quit & Reopen to activate capture access. No broader permissions were added.

## Neutral blur color (0.2.3)

The user reported a brown cast in the faded background. Added an AppKit content filter to the native backdrop only: CIColorControls with saturation 0, brightness -0.025, and contrast 1. The cards and icons remain sibling views outside that filter, preserving their original colors and blue selection outline. The existing fade masks and caption-free layout are unchanged.

- Strict Clippy and formatting pass. Sample render inspected for intact colored icons and selection, no captions, and unchanged feathering. Cached AppKit bitmaps do not sample the live desktop; final color requires the installed overview to be viewed over real windows.
- Release native smoke passes enumeration, native focus, continuous selection/reversal, non-key overview, unchanged geometry, fixed-size positioning, snap/restore, minimum-size anchoring, and event-tap creation.
- Installed 0.2.3; existing Accessibility entry refreshed. The Mac locked during the existing screen capture identity refresh. After the user unlocked it, finished re-adding the verified installed 0.2.3 bundle and used macOS Quit & Reopen. Settings shows screen capture enabled; live color verification is pending.

## Lighter native blur (0.2.4)

The user found 0.2.3 too dark and said it did not feel blurred. Removed the Core Image content filter and switched the visual-effect material from HUDWindow/VibrantDark to UnderWindowBackground/DarkAqua. A faint cool-gray translucent wash lightens the native backdrop without filtering its compositor output. Preview cards/icons stay in a separate sibling view; fading edges and caption removal remain.

- Added `overlay_brightness` (default 0.10, finite range 0–0.35) to control the light wash. Configuration reload applies it on the next overview without rebuilding or renewing permissions. Existing configurations inherit the default. Extended configuration validation coverage for invalid values and missing-field compatibility.
- All 36 tests, strict Clippy, and formatting pass. Final release native smoke passes all focus, geometry, snap/restore, minimum-size, keyboard tap, and non-key overview checks.
- Added opt-in `--preview <path> --hold` for 25-second live compositor inspection. Launched the bundled diagnostic and inspected its native window through computer use: feathering, lighter translucent backdrop, intact blue selection/color icons, and no visible captions. Cached AppKit PNGs are retained only as explicit diagnostic artifacts, not runtime screen capture cache.
- Installed and signature-verified 0.2.4. Refreshed existing Accessibility and thumbnail permissions and restarted with macOS Quit & Reopen. The installed app was running, but the user reported that its background still looked flat. The 0.2.5 implementation below replaces this material path.

## Actual background pixel blur (0.2.5)

The user correctly identified that the material-based versions still looked like a translucent gradient rather than visibly blurred content. Replaced NSVisualEffectView with an explicitly rendered, cropped background capture.

- ScreenCaptureKit requests only the overview's display-local rectangle at 2× resolution. It excludes the entire Magnetic application, refuses capture if it cannot identify its own application for exclusion, hides the backdrop until a real frame arrives, and never captures audio or writes runtime frames to disk.
- Core Image applies restrained desaturation and a 12-pixel Gaussian blur to the actual pixels. Edge clamping avoids dark fringes. The rendered crop retains its original extent, maps back to the same screen rectangle, and uses the existing feathered edges and configurable light wash. Preview cards/icons stay sharp and in full color; captions remain removed.
- A one-shot timer refreshes this small crop at up to five frames per second only while the overview is visible. Only one capture is pending at a time. Closing the overview clears its image and invalidates late results; display/region changes invalidate earlier captures. The idle event-driven window discovery behavior is unchanged.
- All 40 tests pass: 36 existing core/gesture tests plus four backdrop tests. The new pixel-rendering test confirms a sharp black/white edge becomes a smooth transition while distant dark/light regions remain distinct, proving the output is actual blur rather than a flat fill. Lifecycle tests cover no work while hidden, throttling/timeouts, stale frames, and display changes. Strict Clippy and formatting pass; release native smoke passes.
- Installed and signature-verified 0.2.5, refreshed its existing Accessibility and screen capture entries, and restarted. A live diagnostic using the same signed binary successfully produced a 512×1064 crop and was inspected through native computer use: underlying Settings text and icons were visibly Gaussian-blurred, with sharp cards/blue selection above them and soft fading edges. This is direct live rendering validation, not inference from the old material's cached bitmap. Diagnostic windows close automatically; the installed manager remains running.

## Background effect removed (0.2.6)

The user preferred removing the effect unless it could match Notification Center natively. Reviewed Apple's documented NSVisualEffectView materials and NSGlassEffectView; they provide native material/glass containers, but no supported API was found for reusing Notification Center's exact surrounding backdrop. Applied the user's requested fallback.

- Removed the entire backdrop capture/blur module, display-region screenshot requests, Core Image processing, background refresh timer/events, gradient masks, and tint. Removed the direct Core Image dependency and backdrop-specific tests. Only individual-window thumbnail captures remain.
- Restored compact floating cards over a completely transparent panel. Window previews, app icon badges, blue selection outline, individual-window navigation, and no visible captions are retained. Removed unnecessary blur margins and extra panel width.
- Legacy `overlay_brightness` values are accepted and ignored for compatibility; new/generated configurations omit the setting. Configuration tests cover this migration. README and example configuration describe the effect-free overview.
- All 36 remaining core/gesture tests pass. Strict Clippy, formatting, example configuration validation, and final release native smoke pass. Source scan confirms no background-capture event/timer, display crop request, Gaussian filter, or gradient helper remains.
- Installed and signature-verified 0.2.6. Refreshed its existing Accessibility and thumbnail permissions, then restarted. Inspected the final native diagnostic window through computer use: floating cards and icon badges with the selected blue outline, no background material/image/gradient, and no visible captions. The diagnostic exits automatically; the installed manager remains running.

## Subtle thumbnail shadows (0.2.7)

- Added an AppKit `NSShadow` to every thumbnail: 22% black, 6-point blur radius, and a 2-point downward offset. Selected and unselected cards use the same shadow, retaining the blue selection outline.
- Disabled view clipping and supplied a rounded shadow path. Increased transparent panel padding while preserving the cards' screen positions. The shared background remains completely transparent, with no captions.
- Inspected the final live native diagnostic: soft shadows are visible around all four cards. AppKit's view-level shadow is used because layer-only shadow properties were overridden during rendering.
- Formatting, compilation, strict Clippy, and final release native smoke pass. The smoke check covers enumeration, real window focus, continuous selection/reversal, non-key overview, snap/restore, constrained-size anchoring, and the global event tap.
- Installed and signature-verified 0.2.7. Refreshed the existing Accessibility and thumbnail capture identities and restarted using macOS Quit & Reopen. Settings shows Magnetic enabled; one installed manager process is running and the diagnostic has exited.

## Centered 3D overview wheel (0.2.8)

- The selected preview stays at the vertical center of its display, full size and facing forward, including at the first and last entries. Selection-relative geometry progressively reduces scale, adds depth, and rotates around X and Y toward each edge. The unused side at a list end stays empty.
- Persistent cards use 280 ms eased Core Animation transitions. Interrupted transitions restart from the current presentation layer, preserving continuity during reversal. Thumbnail callbacks update the existing image views without restarting motion. Cards entering or leaving the visible working set fade through an extra edge slot; retiring cards finish their transition before removal. Badges share their card's perspective. Transparency, caption removal, selection outline, and subtle shadows are preserved. Reduce Motion removes rotation and animated travel.
- AppKit resets its backing layers' anchors. Visual inspection caught the resulting offset/clipping during development; the final transform explicitly rotates around each card's center while retaining AppKit's zero anchor. Projected-center assertions verify the correction.
- All 38 tests pass (21 acceptance, 15 gesture, 2 wheel geometry tests), with formatting and strict Clippy checks passing. Geometry coverage checks centering across display sizes, symmetric progressive transforms, card/badge separation, edge clearance, and transparent exit slots.
- The opt-in native preview now checks actual compositor interpolation, interrupted reversal, persistent card identity, selection centering, long jumps/list ends, projected centers, settled presentation transforms/opacity, and unchanged-refresh isolation. Final signed release preview passed these checks. Its settled native window was visually inspected: centered selection, symmetric shrinking/tilting neighbors, attached badges, shadows, and no clipping. AppKit's cached bitmap does not represent live 3D transforms and is not used as evidence of perspective rendering.
- Release native smoke passes real AX enumeration/focus, continuous selection/reversal, unchanged selection geometry, fixed-size movement, snap/restore, constrained-size anchoring, event-tap creation, and non-key HUD behavior.
- Installed and signature-verified 0.2.8, preserving a 0.2.7 bundle under tmp/. Refreshed the existing Accessibility and thumbnail capture identities and used macOS Quit & Reopen. Settings shows thumbnail access enabled; one installed manager process remains running. Runtime startup reports one active trackpad listener. Physical-trackpad feel still requires the user's hands-on assessment.

## Opaque wheel, consistent yaw, and foreground badges (0.2.9)

- Removed selection-distance opacity and opacity animation. Every card stays fully opaque, including during travel; edge slots now sit fully outside the panel so cards enter and leave by movement rather than fading. Preview-body and badge fills are opaque. The surrounding desktop remains visible between cards.
- Both upper and lower neighbors now use the same positive Y rotation, turning right progressively. X rotation still follows the vertical wheel direction. Centering, scale, depth, and interrupted-motion continuity are preserved.
- Split each card into a transform wrapper with sibling preview-body and badge views. The preview body owns the rounded border and shadow; the badge renders above it. This removes the parent border that previously crossed the app icon. Border-color/width transitions now animate on the preview body.
- All 38 tests, strict Clippy, formatting, and release native smoke pass. Updated geometry assertions cover matching positive yaw at both ends and fully off-panel exit slots. Native preview checks additionally assert opaque card layers and badge/body sibling ordering with no wrapper border. Live release rendering was visually inspected: opaque cards, consistent rightward yaw, and an uninterrupted app badge in front of the selected blue border.
- Installed and signature-verified 0.2.9 with a 0.2.8 backup under tmp/. Refreshed the existing Accessibility and thumbnail capture identities and restarted through macOS Quit & Reopen. Settings shows thumbnail access enabled; runtime startup reports one active trackpad listener.

## Natural scrolling, wheel input, and release feedback (0.3.0)

- Reversed vertical three-contact selection and edge travel to natural scrolling: fingers down select earlier/upper windows; fingers up select later/lower windows. Horizontal app-local navigation is retained.
- The active public CGEvent tap now includes scroll-wheel events. The contact callback claims eligible three-contact sequences before the navigation threshold, maintains ownership through partial release, and drains that stream's native momentum until its end packet. Fresh ordinary two/four-contact sequences release old ownership; stale/invalid/rejected sequences fail open. Independent physical mouse-wheel packets pass through without clearing a trackpad momentum tail.
- Holding the configured overview modifiers (Control + Option by default) routes mouse-wheel detents and precise scrolling into the same stable navigation session. Owned wheel packets are consumed; unmodified scrolling passes through. Native momentum from an owned precise-scroll stream is drained rather than applied twice, including after modifier release. A fresh unmodified scroll stream does not inherit that ownership.
- Timestamped contact samples estimate release velocity. Longer flicks coast with bounded exponential decay and clamp at list ends. Short strokes select one entry without launching a long fling; pausing before lift stops momentum. Fresh touch, click, keyboard navigation, context/Space change, reload, or event-tap disable interrupts navigation. A one-shot 16 ms timer runs only while coasting; no idle polling was introduced.
- After a short swipe or completed coast, the non-key overview remains visible for 650 ms. Mouse-wheel bursts finish after 110 ms of quiet and use the same feedback delay. New input can interrupt that delay immediately. Existing opaque cards, rightward yaw, foreground badges, and reversible card animation are preserved.
- All 46 tests pass (21 acceptance, 15 gestures, 2 wheel geometry, 8 scroll routing/physics). Formatting, strict Clippy, signed release build, and the native wheel animation diagnostic pass. Tests include a six-second owned native momentum tail, normal two/four-contact isolation, modifier release, fresh streams, reversal, stale samples, bounded coast, and short/held strokes.
- Added a native fixture lifecycle check using the production runtime methods: actual short-swipe focus, kinetic selection, wheel burst completion, non-key visibility through the feedback interval, delayed dismissal, and fresh-contact/Space interruption pass. The diagnostic disables recovery writes and automatic layout and targets only its fixture windows. Existing focus, geometry, snap/restore, constrained-size, and keyboard/scroll event-tap checks pass. One release attempt detected fixture movement while the old installed manager was still arranging windows; stopping that manager eliminated the interference and the full release smoke passed.
- Installed and signature-verified 0.3.0 with a 0.2.9 backup under tmp/. Refreshed existing Accessibility/thumbnail identities and restarted through macOS Quit & Reopen. Settings shows thumbnail access enabled. Physical three-finger feel and raw-contact/scroll ordering still need the user's hands-on assessment; fixture and routing tests do not substitute for hardware swipes.

## Deliberate sustained scrolling and stronger inertia (0.3.1)

- Ordinary swipes are now limited to one slot regardless of initial distance or speed, with no release momentum. Sustained traversal requires continued movement after the overview has been visible for 240 ms, followed by at least 0.45 of a normal step beyond the waiting anchor. The anchor follows early movement, so simply holding still after a quick swipe cannot unlock acceleration or edge repetition.
- Once sustained, finger movement gains up to 3.2× sensitivity according to velocity. Only new movement is integrated when that mode activates; the initial swipe's full distance is not replayed. Edge holding is available only in sustained mode and gradually shortens its repeat interval to 75 ms. Reversal remains immediate.
- Sustained release velocity now supports stronger, speed-dependent launch gain, slower decay for faster swipes, and up to 18 extra slots before the safety limit. List boundaries stop the coast immediately. Stationary edge repeats are excluded from measured release velocity, and pausing before lift still removes momentum. Wheel bursts must also last at least 240 ms before they can coast. The 650 ms feedback delay and scroll interception remain.
- All 51 tests pass (21 acceptance, 17 gesture, 2 wheel geometry, 11 scroll). New cases cover fast/wide ordinary swipes staying at one slot, stationary holds not arming sustained mode, speed-dependent continued movement, stronger/farther/longer fast coasts, and recognizer-to-momentum integration. Formatting and strict Clippy pass.
- The signed release passed native fixture focus, short-swipe feedback, sustained kinetic selection, wheel completion, delayed dismissal, input/Space interruption, geometry/snap/restore checks, and the wheel rendering diagnostic. The old manager was stopped before fixture testing to avoid layout interference.
- Installed and signature-verified 0.3.1 with a 0.3.0 backup under tmp/. Refreshed existing Accessibility/thumbnail identities and restarted through macOS Quit & Reopen. Startup reports one active trackpad listener. Physical gesture tuning remains subject to the user's hands-on assessment.

## Gentler mouse wheel and uninterrupted rapid swipes (0.3.2)

- Mouse-wheel intent now has its own conservative traversal policy. Short bursts stay within one slot. Sustained traversal requires 650 ms of continued input followed by 2.5 additional normalized wheel units beyond the waiting anchor; earlier travel is not replayed. Sustained wheel sensitivity is 0.45 of the previous rate, and its separate release profile caps velocity at 6 slots/second with faster decay and at most one additional selected entry. The stronger sustained-trackpad profile is unchanged.
- Fresh contact now preserves existing feedback rather than hiding the panel before recognition. A 250 ms bridge covers the next swipe's recognition interval, including arrivals near the old dismissal deadline. Starting a new session retains feedback through reconciliation and only clears it after the active session is ready. The 650 ms dismissal delay restarts after each completed stroke.
- Rapid strokes retain the existing panel, cards, thumbnail cache, and in-flight animations. A recent unchanged context is reused; pending discovery work still triggers reconciliation. Every separate ordinary swipe resets recognition intent and continues to select exactly one entry without accumulating acceleration.
- All 54 tests pass (21 acceptance, 18 gesture, 2 wheel geometry, 13 scroll). New cases cover late wheel unlocking, reduced wheel travel, immediate reversal after short-burst overshoot, restrained wheel inertia compared with trackpad inertia, and eight rapid separate strokes without sustained-intent carryover. Formatting and strict Clippy pass.
- The signed release native check exercised six rapid alternating fixture swipes, asserting exact selection and an unchanged panel-hide counter through fresh contact, session restart, and completion. Both reconciliation and recent-context reuse paths passed without a transient hide. The fixture probe excludes non-fixture applications from discovery and disables recovery writes/automatic layout. Existing focus, feedback, dismissal, geometry, snap, input-tap, Space interruption, and wheel rendering diagnostics pass.
- Installed and signature-verified 0.3.2 with a 0.3.1 backup under tmp/. Refreshed existing Accessibility/thumbnail identities and restarted through macOS Quit & Reopen. Startup reports one active trackpad listener. Hardware mouse sensitivity and physical rapid-swipe feel remain subject to hands-on assessment.

## Clickable overview thumbnails (0.3.3)

- Clicking a thumbnail or its app badge directly focuses that window and moves the centered selection to it. The click interrupts gesture/wheel motion and restarts the 650 ms feedback interval. Reconciliation preserves the panel and running card animations. Stale context or removed-window selections are rejected.
- The event tap hit-tests the current Core Animation presentation layers, including perspective and in-flight movement. Only visible, current cards are targets. Owned clicks consume their drag and release, including outside the panel; empty areas and the hidden overview pass input through. Accessibility work remains on the queued runtime path, outside the tap.
- All 54 portable tests and strict Clippy pass. The release native preview verifies moving/tilted thumbnail and badge targets, empty-area passthrough, hidden-panel passthrough, and drag/release ownership. Native fixture checks verify actual AX focus, selection, scroll interruption, continuous non-key feedback, and rejection of stale targets. Existing native layout, scrolling, and rendering checks also pass.
- Modified Rust files pass formatting. The repository-wide `cargo fmt --check` flags an unrelated pre-existing indentation edit in `src/config.rs`, which is preserved and excluded from this change.
- Installed and signature-verified 0.3.3, retaining a 0.3.2 backup under tmp/. Refreshed the existing Accessibility and thumbnail capture identities through System Settings and restarted using Quit & Reopen. Both permissions show enabled; one installed manager process is running.

## Ordered thumbnail click navigation (0.3.4)

- A distant thumbnail click now visits each intervening window in the existing stack order. Clicking window 4 from window 1 visibly focuses 2, then 3, then 4; reverse travel follows 4, 3, 2, 1 without wrapping. One-shot timers separate steps by 140 ms, and an overdue timer still advances only one window. The wheel retargets from its current presentation and stays open throughout; the 650 ms feedback interval starts after arrival.
- A second click replaces the route from the current selection. Touch, wheel, keyboard input, outside mouse input, and Space changes interrupt travel. Failed focus, external focus changes, or removal of any remaining target stop the sequence. Automatic layout waits until travel finishes.
- All 57 portable tests and strict Clippy pass. Added coverage for forward/reverse intermediate selection, unchanged stack order, adjacent/current clicks, retargeting, unfocused contexts, failed focus, external focus, Space changes, and removed windows. Modified files pass formatting; the repository-wide check still flags the preserved unrelated indentation edit in `src/config.rs`.
- Expanded native smoke fixtures to four windows, positioned within the display so the existing snap/restore checks remain valid. Native checks confirm actual AX focus at every forward and reverse step, enforce the visible interval and one-step behavior for late timers, and verify continuous non-key feedback, retargeting, touch/keyboard interruption, and final feedback. Existing native input, geometry, snap/restore, and animated click hit-testing checks pass.
- Installed and signature-verified 0.3.4 with a 0.3.3 backup under tmp/. Refreshed existing Accessibility and thumbnail capture identities through System Settings. Both permissions show enabled; one installed manager process is running and its recovery state was refreshed after startup.
