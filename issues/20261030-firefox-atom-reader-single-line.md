# In Firefox, Markdown Stress's single-mode atom reader is a line taller than in Chrome

**Status:** Open
**Systems:** web host reader pills (`host/web/atoms.js`), the JS target in Firefox, cross-browser conformance (`host/web-js/conformance/known-firefox.json`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from #1148's conformance runs
**Date:** 2026-10-30

## Symptom

After `tap toggle-single`, Markdown Stress's left-to-right atom reader (`atom-reader`, `atomText` drawn with `atoms="citation mention skill"`) is 133 px tall in Firefox and 107 px in Chrome. `conform.mjs markdown-stress --browser firefox --strict` fails on `layout.atom-reader.h` and, below it, `layout.markup-node.y` at each of the five single-mode steps: `tap toggle-single`, `tap markup-editor`, `tap format-bold`, `tap format-italic` and `tap format-heading`. `known-firefox.json` names neither.

## Measured

- On origin/main at `78ae0e76ac`, with `--build --build-js` and a wasm root of its own: `atom-reader.h` is 107.00 in Chrome and 133.00 in Firefox at all five steps, and `markup-node.y` is 1003.39 against 1028.40 (1058.39 against 1083.40 at `tap format-heading`).
- The reader's `x`, `y` and `w` agree within the 4 px tolerance, so its box is as wide in both. At its 26 px line height, Firefox breaks its text into one more line.
- WebKit reports no difference on either box (#1148's branch, where this reader is main's).
- The reader is drawn only in single mode (`if single` in `apps/markdown-stress/app.contract`). It came with #997. Its text is two paragraphs with six pills and no icons, among them two citations with no space between (`[B](cite:2)[C](cite:3)`).

## Cause

A Firefox engine difference, not the web host's: in macOS's system font (`system-ui`), Firefox gives each word space 0.31 px more advance at 16 px than Chrome and WebKit do, and the reader's first line is within 1.42 px of its width.

- At the conformance viewport (420 × 900) the reader is 380 px wide. Its first line is `Ask`, the citation pill, `about`, the mention, `and`, the skill and `here.`, with six word spaces. Range client rects put its last character's right edge at 378.58 px in Chrome; in Firefox the line would need 380.62 px, so `here.` wraps onto a second line. The second paragraph breaks at the same places in both engines.
- Every space on that line is 4.19 px wide in Chrome and 4.50 px in Firefox; every letter, and each pill's width apart from the space inside `Assistant quote`, agrees within 0.05 px. Pills, the white space between them and their `vertical-align: bottom` play no part.
- Plain HTML, `font: 16px system-ui`, outside Exact: a lone space in its own span is 4.19 px in Chrome and WebKit and 4.50 in Firefox. Under `font-kerning: none` (or `font-feature-settings: 'kern' 0`), or as a no-break space, Firefox's is 4.19 too. Helvetica, Arial, Georgia and `monospace` agree across all three engines within 0.03 px. So Firefox applies an adjustment from the system font's kerning to the word space that Chrome and WebKit (Core Text, the platform's own text stack) do not.

Working around it would mean sniffing the engine or turning off kerning, so the ten differences (`atom-reader.h` and `markup-node.y` at each of the five steps) are recorded in `known-firefox.json` at their measured pairs.

## Notes

- The five Firefox `layout.reading-column.h` entries in `known-firefox.json` are measured with this extra line in the column. If Firefox's word space comes to match, each moves by 26 px.
- #1148 puts its right-to-left fixture above this reader, so that none of its boxes moves with this difference.
- Repro: `bun host/web-js/conform.mjs markdown-stress --browser firefox --strict --build --build-js --wasm-root /tmp/<yours> --out /tmp/<yours>-out`, with Homebrew's OpenSSL first on `PATH` (`docs/agent-pitfalls.md`).

## 2026-11-10: cause found, differences recorded

- Measured on `cdacafce`, with `--build --build-js`, Firefox 157.0 (Playwright 1.64.1's) and its Chromium. `markup-node.y` is now 1236.39 in Chrome and 1261.40 in Firefox (1291.39 against 1316.40 at `tap format-heading`), since #1148's right-to-left fixture sits above the reader; `atom-reader.h` is unchanged.
- The ten differences are in `known-firefox.json` under one reason, and `conform.mjs markdown-stress --browser firefox --strict` passes. The five `layout.reading-column.h` entries stay as they are: the extra line is the engine's, so they measure what Firefox draws.
- The older reason those entries and most other Markdown Stress entries share says each engine maps a generic family to its own face, with system-ui 16 px measured at 459.9 px in Chrome and WebKit and 462.9 in Firefox for 60 characters. For `system-ui` the face is the same; the 3 px is consistent with this wider word space (about ten spaces at 0.31 px). `ui-monospace`, which only WebKit maps to SF Mono, is a different face.
- Left open for a decision: report the space advance to Firefox, or close this as an engine difference that is recorded. Nothing in Exact needs to change for either.
