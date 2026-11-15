# A native header or sheet title ignores the heading's authored font

**Status:** Closed
**Resolution:** A navigation bar's title (iOS) and a modal route's sheet title (macOS) take the font-family, font-weight, font-style and font-size written on the header's heading; each row left unsaid stays the platform's
**Systems:** Apple host navigation bar projection, docs
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

A sheet's heading written `font-family="Filson" font-weight=800` draws in UIKit's regular navigation-title font. Brooks' source uses its brand face for every bar title.

## Cause

The native bar's title is projected from the heading's text only; its authored typography is not carried to `UINavigationBarAppearance`. The port sets the font from its own `navigation(_:)` hook.

## Fix

Project an authored `font-family`, `font-weight` and `font-size` on the header's heading into the bar's `titleTextAttributes` and `largeTitleTextAttributes`, as CLAUDE.md's precedence asks (what the author writes wins). Until then, document the restriction beside native headers.

## Resolution

`NodeView.authoredTitleFont` (`host/apple/Sources/ExactKit/HeaderRule.swift`) takes the
platform's title font and replaces each of `font-family`, `font-weight`, `font-style` and
`font-size` that the author wrote on the heading. A row the heading only inherits is not
replaced, nor is one the compiler gave it. The author's size is the kernel's number, so Dynamic
Type is already in it.

- **iOS.** `NavigationTitleInk` (`NavigationTitleInkIOS.swift`), which already kept an authored
  title colour in the route's own item appearances, now also sets `.font` in
  `titleTextAttributes` over UIKit's `headline` and in `largeTitleTextAttributes` over its bold
  `largeTitle`. A level-1 heading's size is the large title's, and its collapsed inline title
  keeps UIKit's size. Any other heading's size is the inline title's. A heading with none of the
  four rows takes the old path, which leaves the item's appearances nil. tvOS is unchanged,
  because its bar title font is not measured.
- **macOS.** A modal route's sheet title row (`RouteChrome`, `RouteSheetMac.swift`) takes the
  font over its bold system font. The window title, which a header's heading becomes, keeps
  AppKit's font, since no public API sets it.
- The native fixture's headings wrote `font-weight=700 font-size=20`, which the bar used to
  ignore. They now write nothing, so the fixture's bars stay UIKit's.

Not done: on iOS, a heading inside a pressable title group is drawn by `HeaderTitleView`, whose
title stays `.headline` (QUEUE.md). `docs/contract-for-agents.md` ("Type: say a heading, not a
size") says what carries and what does not.

## Verification (2026-11-10)

- On the iOS 27.0 simulator (iPhone 17), with a scratch app whose large, inline and sheet headings
  wrote `font-family="Georgia"` with a weight or a size, `agent ios --chrome platform` screenshots
  before the change showed San Francisco in all three. After it, all three are Georgia.
  A plain heading's bar screenshot is byte-identical before and after.
- `RouteSheetMacTests.testTheTitleRowTakesItsHeadingsAuthoredTypography` passes (Menlo 18 bold,
  a weight alone at the system size, and the platform's font for a plain heading), as does the
  rest of `RouteSheetMacTests`. `HeaderMacTests.testASearchInputIsTheBarsSearchItemHoldingTheInputsOwnField`
  fails here with and without this change.
- `NavigationBarIOSTests.testAHeadingsAuthoredTypographyIsTheBarsTitleFont`
  (`NavigationTitleInkIOSTests.swift`) checks each of the item's four appearances, the large
  title UIKit draws, a family alone and an inherited font. It compiles for the simulator
  (`EXACT_TESTS=1 swift build --target ExactKitTests` for `arm64-apple-ios17.0-simulator`), but
  this Mac could not run it: `xcodebuild` refuses every simulator because CoreSimulator
  (1174.9.2) is older than Xcode 27.2's (1174.21.0), and updating it needs an administrator.
  `build.mjs --tvos --no-install` compiles the shared tvOS code.
