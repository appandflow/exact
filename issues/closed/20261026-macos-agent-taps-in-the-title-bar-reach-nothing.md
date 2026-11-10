# macOS: an agent tap on a node under the title bar reaches nothing

**Status:** Closed
**Resolution:** Does not reproduce at cdacafce: the window's hit test finds the page's title-row button, and agent taps and application clicks press it; TitleRowTapMacTests guards it.
**Systems:** Agent driver (macOS), Apple host (`window-controls-overlay`)
**Author:** Unrecorded (found driving the T3 Code clone)
**Date:** 2026-10-26

**Problem.** In a `window-controls-overlay` app (`display_override`, LLP 1039 D8), the page draws its own title
row under the title bar. `scripts/agent.mjs macos` taps on that row's buttons reply `delivery: "platform"`, but
the page gets no `pointerdown`, `pointerup` or `press`: the journal logs nothing for them and their state does not
change. A tap on a node below the title bar, in the same drive, logs all three. Reproduced with the T3 Code clone
(`examples/t3-code` on `daehyeon/t3-code-main` 6ead721537, window 1280 × 840): `tap toggle-right-panel` and
`tap toggle-terminal` in the chat header change nothing (`state rightPanel` stays `false`), and `tap new-thread`
in the sidebar logs `pointerdown`, `pointerup` and `press view 290`. The row's commands still run by key.

**Cause.** The platform tap (`host/apple/Sources/ExactKit/Mac/AgentMac.swift`, the `NSApp.sendEvent(down)` path)
sends a `leftMouseDown` at the node's window point. With the overlay, the window has an empty unified `NSToolbar`
(`host/apple/Sources/ExactMac/main.swift` `overlayToolbar`) and a full-size content view, so the title bar's
own views lie above the page in the title bar band. The window's hit test finds the title bar there, not the
page's view, and the agent's window is never key, so AppKit treats the click as one on the title bar. Whether a
person's click on a key window reaches the button was not checked.

**Fix.** The agent's tap on a node under the title bar should reach that node, as one below the title bar does:
deliver to the node's view when the title bar's hit view is not a control, or hit-test the page first in the
overlay's band. Check a person's click on the key window as well; if it is lost too, the overlay needs the same
fix for real events. A host test: a `window-controls-overlay` app whose title-row button counts presses, tapped by
the agent.

## Resolution

Nothing to fix at `cdacafce`. In a window set up as `coverChrome` sets one up for
the overlay (full-size content, a transparent title bar and an empty unified
toolbar), the theme frame's hit test at a title-row button returns the page's
view for that button, not a title-bar view. The empty part of the band hits
the page's own box too. The agent's tap therefore reaches the button through
`NSApp.sendEvent`, as one below the title bar does. A person's click takes the
same route: AppKit routes it by the same hit test, which does not depend on
whether the window is key. What changed since the report is not known (the
history is squashed). No host fix for real events was needed.

`host/apple/Tests/ExactKitTests/TitleRowTapMacTests.swift` is the requested
host test. Its app has a title row with a button at each end, sized by
`env(titlebar-area-*)`, and a button below the row. The test asserts that the
window's hit test at each title-row button finds that button. It then sends
agent taps that must arrive with `delivery: "platform"`, and checks the press
counts. Last, it sends a click through `NSApp.sendEvent` outside the agent,
which must press the button too.

## Verification

- A scratch `window-controls-overlay` app (`exact new`, no routes), driven with
  `bun exact.mjs agent macos --size 1280x840` on macOS 27.0.1. It has a
  sidebar header beside the traffic lights holding an icon button, and a chat
  header in the title row with `toggle-right-panel` at its right end. Both taps
  answered `delivery platform`, `logs` showed `press view 4 (pressTop)` and
  `press view 11 (toggle)`, and `state rightPanel` read `true`. Earlier
  versions pressed too: three buttons spread across the row at the same size,
  and one beside the traffic lights at 900 × 600.
- `bun host/apple/build.mjs --test --only TitleRowTapMacTests` passes.

Not verified: a person's hardware click, and dragging the window by the empty
part of the title row. The screen was locked for the whole session, so
neither could be done by hand or sent through System Events. The XCTest
process cannot make its window key. Since the empty band hits the page's box
and not the title bar, whether AppKit still drags the window from there should
be checked by hand.
