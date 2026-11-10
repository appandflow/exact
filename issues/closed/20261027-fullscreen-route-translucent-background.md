# A full-screen route that paints a translucent background covers what is under it

**Status:** Closed
**Resolution:** A full-screen route whose author wrote its background stands over the page it covers on iOS, macOS and the web
**Systems:** navigation (Apple, web)
**Severity:** P3
**Author:** Claude (Opus 5.5), from Excord's image viewer
**Date:** 2026-10-27

Ruling (Charlie, 2026-11-10): option A. A full-screen route that paints its own background
leaves the host surfaces behind it clear, so a translucent background, or one animating
toward transparent, composites over the page it covers, as CSS has it. A route that paints
nothing keeps the platform's background, unchanged. A route that paints an opaque colour is
unchanged in what you see.

## Symptom

Excord's image viewer is a full-screen route (`navigationPresentation: modal`, full
screen) whose column paints `#000` and, on a drag down, animates that background toward
transparent. Discord's apps and Photos fade the black away to show the dimmed channel
under the picture. Here nothing under the route shows:

- iOS: the route's controller paints the route's background again
  (`RouteController.paintBackdrop`), and the presentation keeps the system background
  under it (`ModalController.paintSurface`). In dark mode the fade is invisible (black
  over black); in light mode it goes from black to white. The colour is drawn twice, so
  at half its alpha the screen is three-quarters dark.
- Web: the fade goes to the host's grey under the route.

## Background

A full-screen route covers the screen, so each host puts an opaque surface under it: the
platform's background where the route paints nothing (LLP 1115 D2). An author can already
ask for a clear route with `background: transparent` (LLP 1141.001 D3, for sheets). A
translucent background, or one animating toward transparent, is neither: today it lands
on the opaque surface. In CSS a translucent box shows what is behind it, which here is the
page the route covers.

## Options

- **A. A full-screen route that paints its own background leaves the surfaces behind it
  clear** (iOS: the presentation surface and the route controller's view, about four
  lines in `ModalIOS.swift` and `NavigationIOS.swift`, and an XCTest; the web host
  likewise). The author's translucent black then composites over the page underneath,
  as CSS has it. A throwaway build showed the dimmed `#general` through the fade in both
  appearances. A route that paints nothing keeps the platform's background.
- **B. Paint the colour once and keep the opaque surface.** The fade's curve becomes
  right, but it still fades to black or white, not to the channel.
- **C. Leave the hosts; the app writes `transparent` on the route and fades an inner
  backdrop.** That backdrop cannot reach the screen's edges until LLP 1123 D4's
  `-exact-safe-area` exists.

## Recommendation

A. It follows LLP 1115's precedence (what the author writes, a translucent background,
wins, and CSS composites it over what is behind), it is what a hand-built UIKit viewer
does (an `overFullScreen` presentation over a clear view; the host already presents a
full-screen route `overFullScreen`), and it changes nothing for a route that paints an
opaque colour or none. Confidence: about 80%. The doubt: a route that paints a translucent
colour at rest, not only during a fade, would show the page under it too, which is what
CSS says and may surprise an author who expected the platform's background.

## Resolution

Option A, on every host that presents a full-screen route. "Paints its own background"
means its author wrote `background-color` (`transparent` included) or `background-image`.

- iOS: `NodeView.paintsOverPage` says so of such a route; `ModalController.paintSurface`
  and `RouteController.paintBackdrop` then leave their views clear, so the route draws its
  colour once, over the page `overFullScreen` keeps in the window. A route that paints
  nothing keeps both surfaces as before.
- macOS: the route beneath stays shown, and inert, under such a route, as under a modal
  route painted in the window (`NavigationMac.sync`). The Mac never drew the colour twice;
  it hid the page, so the half-alpha black showed the window background.
- Web: `neutral.css` gives a full-screen route a clear colour no author writes, and
  `navigation.js` `overPage` reads the computed colour to tell a route whose author wrote
  one; the page under it stays shown and inert, in `project` and in `document.js`
  `markRoutes` before startup.

A bound background that evaluates to `""` is an explicit clear on Apple (LLP 1115.000 D2),
so there the route stands over the page; the web JS target drops the row, so there it
paints nothing (QUEUE.md).

## Verification (2026-11-10)

A scratch app (`exact new`, not committed): a page of coloured boxes and four full-screen
routes, `dim` (`rgba(0, 0, 0, 0.5)`), `opaque` (`#2a6`), `bare` (no background) and `fade`
(`#000` to `transparent` over 3 s).

- iOS 27.0 simulator (iPhone 18 Pro), `agent ios` screenshots, light and dark: `dim` shows
  the page at half brightness through the black, to the screen's edges; `opaque` fills the
  screen; `bare` is the system background, white and black; `fade` at 1.5 s shows the page
  half dimmed, and at its end the page whole.
- macOS, `agent macos`, light and dark: the same three results.
- Web, `agent web`, light and dark: the same, `fade` included. Before the change `dim` was
  flat grey there.
- `ModalSurfaceIOSTests` (new) covers the three cases over `navigation-bars.contract`. It
  did not run: this Mac's CoreSimulator (1174.9.2) is older than Xcode 27.2 requires, so
  `xcodebuild` sees no simulator and `build.mjs --test --ios` stops before building, and
  the update needs an administrator. The async lane runs it.
- macOS XCTests `BackMacTests`, `HeaderMacTests`, `ModalGroundMacTests`,
  `RouteSheetMacTests`, `RouteSheetPushMacTests`, `SheetSurfaceMacTests`, `SplitMacTests`,
  `TitleBarAreaMacTests`: 41 run, 2 failures (`HeaderMacTests` search item,
  `SheetSurfaceMacTests` dark ground), which fail the same with the old `NavigationMac.swift`.
- Bun: the navigation, modal, sheet, document, look and bundle suites of `host/web` and
  `host/web-js`, and `apps/shared-elements` (a full-screen photo route): all pass but
  `sheet-ids.test.mjs`, which fails the same with the old `neutral.css` (QUEUE.md).
- `build.mjs --tvos --no-install` builds; `caps.mjs`: all budgets within cap.
