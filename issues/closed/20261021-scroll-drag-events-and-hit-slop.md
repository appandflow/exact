# Scroll events carry no drag start, end or velocity, and a control has no hit slop

**Status:** Closed
**Resolution:** scrolldragstart and scrolldragend with a ScrollDragEvent (2aa560c2) and -exact-hit-slop (ec2849c1) landed; Android's Kotlin scroller does not report drags yet (QUEUE)
**Systems:** Contract scroll events, Apple host scroll delegate, hit testing
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' collapsing header hides on scroll and comes back on an upward flick. From `scroll` (x and y) and `scrollend` (no payload) the port mistook a bounce at the bottom for a flick. It attached its own target to the `UIScrollView`'s pan recognizer from a route hook to learn when a drag began and how fast it ended. Small icon buttons copied at the source's size are harder to hit than the source's, which pads them with `hitSlop`.

## Fix

Report drag start and drag end, with velocity, as typed scroll events, matching what `UIScrollViewDelegate` gives (`scrollViewWillBeginDragging`, `scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`). Let a control grow its hit area beyond its box without changing layout, as a property the web host maps to a padded hit region.

## 2026-11-10: hit slop landed

The hit-slop half landed in ec2849c1 (Charlie's ruling, 2026-11-10: build it). `-exact-hit-slop` takes one to four nonnegative lengths as `padding` does, with `-exact-hit-slop-top`, `-right`, `-bottom` and `-left` for one side; React Native's `hitSlop` written as an attribute is redirected to it. It grows where a pointer reaches the node and moves nothing: layout, paint and accessibility never see it, and `tap <target>` still aims at the box's middle. The front box still takes the pointer where boxes overlap, slop and all, and an `overflow` other than `visible` on the node or an ancestor clips it. The kernel's hit walk (`elementFromPoint`), iOS and macOS `NodeView.hitTest`, and the Linux presenter (which Windows and Android share) grow the box; the web draws the slop as a positioned pseudo-element, which reaches from the padding edge, and refuses it on replaced elements.

## 2026-11-10: the scroll half landed

Commit 2aa560c2 adds `scrolldragstart` and `scrolldragend` on a scroller
(Charlie, 2026-11-10), named for the delegate calls they report:
`scrollViewWillBeginDragging` and
`scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`. The start
takes no payload. The end's action may take a `ScrollDragEvent`: `velocityX` and
`velocityY` in UIKit's units, points (CSS px) per ms, positive as `scrollLeft`
and `scrollTop` grow, and `targetLeft` and `targetTop`, `some` where the scroll
will rest when the host knows it before the momentum runs, else `none`.

iOS reports UIKit's velocity and target (after any snap). macOS takes a
trackpad drag, with the velocity from its last 100 ms and no target. The web
takes a touch drag, with the finger's velocity over its last 100 ms of moves
and no target. Linux and Windows have no scroll drag and send neither; the
presenter they share with Android takes Android's scroller reports, which its
Kotlin does not send yet (QUEUE). A wheel, the keyboard and a programmatic
scroll send neither. `docs/contract-grammar.md` (events) and
`docs/reference.md` ("Scroll drags and swipes") document them.

A web drive on both targets measured 1.00 px/ms for a finger moving 200 px in
200 ms, 0.50 for one moving 150 px in 300 ms, and 0 for one that rested
300 ms before lifting.
