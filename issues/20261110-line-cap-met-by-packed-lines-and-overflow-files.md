# Files at the 1,500-line cap stay under it by packing lines, and code is placed where there is room

**Status:** Open
**Systems:** host/web/glue.js, host/web/navigation.js, host/web-js/rt.js, scripts/agent.mjs, js/src/prelude.js, js/src/binary.js, host/web-js/build.mjs
**Severity:** P3
**Author:** Claude (Fable 5.2), from the 2026-11-10 project review
**Date:** 2026-11-10

## What is wrong

`scripts/caps.mjs:166` counts newlines, so it cannot see how much a line holds. At `dd6f5dcd`, of the
7,562 files it scans, 121 hold 1,450 lines or more, eight hold exactly 1,500 and none holds more. Each
50-line band from 1,000 to 1,449 holds 22 to 38 files; the band from 1,450 to 1,500 holds 121.

Rust files meet the cap honestly, because `rustfmt` bounds their lines (`kernel/src/arena.rs`, 1,499
lines, none over 100 characters). JavaScript has no formatter here, and its files meet the cap with
longer lines. "Reprinted" is the count of code lines after Bun's printer (`bun build --no-bundle`)
puts one statement on a line and drops the comments.

| File | Lines | Over 200 chars | Longest | Code lines, reprinted |
| --- | ---: | ---: | ---: | ---: |
| `host/web/glue.js` | 1,500 | 185 | 1,699 | 1,342 to 3,259 |
| `host/web-js/rt.js` | 1,485 | 145 | 1,141 | 1,325 to 3,073 |
| `scripts/agent.mjs` | 1,486 | 139 | 6,756 | 1,250 to 1,880 |
| `host/web/navigation.js` | 1,500 | 100 | 2,567 | 1,354 to 2,390 |
| `js/src/prelude.js` | 1,500 | 41 | 713 | 1,112 to 1,807 |
| `scripts/deploy.mjs` (for comparison) | 1,496 | 29 | 409 | 1,300 to 1,630 |

Across the tree 0.46% of lines pass 200 characters; in `glue.js` 12.3% do.

The cap also decides where code lives:

- `host/web/navigation.js` holds list rows, history, the focus controller, fold and preference facts,
  text fitting, the page store, field selection, grant admission, text selection and scroll drags;
  `glue.js:2` imports 60 names from it. `scripts/boot.mjs:13` allows only these two modules before
  first pixel, so what one cannot hold goes to the other.
- `host/web-js/build.mjs:529-541` makes grant admission a module by cutting the text between
  `// grant admission: begin` and `end` (`navigation.js:1239`, `:1448`), and throws if the section
  before it is not adjacent.
- `js/src/binary.js:5-7`, a file named for bytes, holds "an image call's options and an answer's
  timers (here, beside the prelude, under its line cap)".
- `scripts/agent-launch.mjs:469-471` passes `--reader` through `process.env.EXACT_ANDROID_READER`
  because "the driver's own file is at its line cap".

## The failure

`issues/closed/20260927-files-at-line-cap.md` reported this for seven files and was closed by splitting
22. Fifteen of the 22 are within 50 lines of the cap again: `glue.js` 1,427 to 1,500, `scripts/agent.mjs`
1,266 to 1,486, `kernel/src/txn.rs` 1,122 to 1,499, `host/linux/src/surfaces.rs` 1,021 to 1,495. That
issue cited "300-character one-liners" in `glue.js`; it now has 108 lines over 300 characters. Its fix
for `glue.js` moved the guest-frame code into "existing `navigation.js`".

## Verification

Counted every figure above with a script over the file set `caps.mjs` scans, reprinted the six files
with `bun build --no-bundle`, and compared the closed issue's 22 files with their line counts now.

## Direction

Split the five JavaScript files by concern and write one statement to a line, as the earlier fix did.
Grant admission becomes a real module both web targets import, which deletes the text cut in
`build.mjs`. What the first pixel does not need can load as `afterPaintPieces` (`navigation.js:491`)
loads the motion and collection pieces, outside boot's two modules. What it does need can only leave
`glue.js` and `navigation.js` if `boot.mjs:13` names a third module, which is the owner's call.

## Related

`issues/closed/20260927-files-at-line-cap.md` (regressed, above). `issues/closed/20261025-a-large-root-component-cannot-meet-the-line-cap.md`
is about an app's root component, not the host's files.
