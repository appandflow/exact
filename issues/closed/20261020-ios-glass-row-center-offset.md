# A padded glass row with align-items=center places its children 3 pt high on iOS

**Status:** Closed
**Resolution:** Does not reproduce on cdacafce5d: on the iOS 27.0 simulator the issue's row and its variants (with and without backgroundMaterial, a plain box, symbol, two-line and glass-styled buttons, align-items flex-start) place the 48-point child at y 4, as Chrome does
**Systems:** kernel (layout), Apple host (iOS), materials
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

The bottom bar of the Linear port:

```
row height=56 padding=4 box-sizing="border-box" backgroundMaterial="glass" align-items="center"
  button height=48 …
```

`agent ios "layout tab-inbox"` put the 48 pt buttons at (4, 1) in the row, not (4, 4). A button
beside the row with no padding sat where expected. `align-items="flex-start"` puts them at y 4.

## Expected

CSS: the row's content box is 56 − 2 × 4 = 48 pt tall, so a 48 pt child centred in it starts
at y 4. A y of 1 is what a child 54 pt tall gives, (48 − 54) / 2 + 4, so the first thing to find is
whether the button lays out taller than its `height`. Compare the same tree in Chrome (the
layout oracle) and on iOS, with and without `backgroundMaterial`, before changing the kernel.

## Reproduction

The row above holding one `button height=48`; `agent ios "layout <button>"`.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-20),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet reproduced on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` (entries 21 and 22) on Charlie's Mac.

## Resolution

Closed as not reproducing. On `cdacafce5d` the kernel and the iOS host agree with Chrome on
every tree below, so neither was changed. The Linear port's source and the lane's notes are no
longer on this Mac (`~/lanes/linear` is gone), and `9b2765ab79` is not in the squashed history,
so the original tree could not be replayed. What its `button height=48 …` elided may have mattered.
If the offset comes back, file it again with that button's whole source, which can then be
diffed against these variants.

## Verification (2026-11-10)

A scratch app holding each row below at `height=56 padding=4 box-sizing="border-box"`,
driven with `agent web` (Chrome, the layout oracle) and `agent ios` on the iOS 27.0 simulator
(iPhone 17). `layout <child>` gave the child's frame in its row:

| Row | Child (48 points tall) | Chrome | iOS |
| --- | --- | --- | --- |
| glass, `align-items="center"` | `button height=48` with a text | 4, 4 | 4, 4 |
| no material, `background-color` | the same button | 4, 4 | 4, 4 |
| glass, centred | `column height=48 width=48` (not a button) | 4, 4 | 4, 4 |
| glass, centred | `button height=48 width=48` with a symbol | 4, 4 | 4, 4 |
| no material | the same symbol button | 4, 4 | 4, 4 |
| glass, `align-items="flex-start"` | `button height=48` with a text | 4, 4 | 4, 4 |
| glass, `border-radius=28` | two `button height=48 flex=1` tabs, a symbol over a 10-point label | 4, 4 | 4, 4 |
| the same, `position="absolute"` at the bottom | a tab as above and a `buttonStyle="glass"` button | 4, 4 | 4, 4 |
| glass, at the bottom | native buttons with a symbol and a label, in a row and in a column (one whose face overflows 48 points) | 4, 4 | 4, 4 |

The screenshots show each child's box drawn where `layout` puts it. A `role="tablist"` row is
not this case: on iOS it becomes UIKit's tab bar, 83 points tall, with its tabs centred in it.
