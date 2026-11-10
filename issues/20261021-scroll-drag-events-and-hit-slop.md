# Scroll events carry no drag start, end or velocity, and a control has no hit slop

**Status:** Open
**Systems:** Contract scroll events, Apple host scroll delegate, hit testing
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' collapsing header hides on scroll and comes back on an upward flick. From `scroll` (x and y) and `scrollend` (no payload) the port mistook a bounce at the bottom for a flick. It attached its own target to the `UIScrollView`'s pan recognizer from a route hook to learn when a drag began and how fast it ended. Small icon buttons copied at the source's size are harder to hit than the source's, which pads them with `hitSlop`.

## Fix

Report drag start and drag end, with velocity, as typed scroll events, matching what `UIScrollViewDelegate` gives (`scrollViewWillBeginDragging`, `scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`). Let a control grow its hit area beyond its box without changing layout, as a property the web host maps to a padded hit region.

## 2026-11-10: hit slop landed

The hit-slop half landed in a072d7de (Charlie's ruling, 2026-11-10: build it). `-exact-hit-slop` takes one to four nonnegative lengths as `padding` does, with `-exact-hit-slop-top`, `-right`, `-bottom` and `-left` for one side; React Native's `hitSlop` written as an attribute is redirected to it. It grows where a pointer reaches the node and moves nothing: layout, paint and accessibility never see it, and `tap <target>` still aims at the box's middle. The front box still takes the pointer where boxes overlap, slop and all, and an `overflow` other than `visible` on the node or an ancestor clips it. The kernel's hit walk (`elementFromPoint`), iOS and macOS `NodeView.hitTest`, and the Linux presenter (which Windows and Android share) grow the box; the web draws the slop as a positioned pseudo-element, which reaches from the padding edge, and refuses it on replaced elements. The scroll half (drag start, drag end and velocity) is still open.
