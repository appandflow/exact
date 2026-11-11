# Return in a list item's bold text or link strips the format, leaves the link's destination as text, and puts the caret one character late

**Status:** Open
**Systems:** markdown/editor/src/editor.rs, markdown/src/edit.rs
**Severity:** P1
**Author:** Claude (Fable 5.2), from the 2026-11-10 project review
**Date:** 2026-11-10

## What is wrong

Return in a list item splits the item at the caret, and in a heading breaks the line
(`newline`, `markdown/src/edit.rs:513` at `dd6f5dcd`). When the caret is inside an inline
pair or a link's label, each half keeps part of the syntax and the styler no longer hides it.
`Editor::simulate` then removes, as orphans, every code unit that was hidden, is now visible
and is one of ``*_~`[]()#>-+.! `` (`markdown/editor/src/editor.rs:175`, `:944`). For a link
that is its brackets and the punctuation of its destination; the destination's other
characters stay as text.

`Editor::newline` tries the caret and the position after hidden closers (`editor.rs:1213` to
`:1222`), never the one before a hidden opener, and a caret at the start of an item whose
text begins with a pair or a link sits after the opener. So Return there splits inside the
construct too.

With orphans removed, `newline` cannot use the selection `edit` returned and takes
`sim.caret(a + 1)` (`editor.rs:1229`): a position one code unit past the old caret, which
lands after the first code unit of the new item's text. When that character is outside the
BMP the caret is inside its surrogate pair.

## The failure

In the web editor, in Chromium: click where the table says, press Return.

| source | caret | after Return | caret after |
| --- | --- | --- | --- |
| `- [docs](https://example.com) here` | start of the item | `- \n- docshttps://examplecom here` | after `d` |
| `- **Note:** details` | start of the item | `- \n- Note: details` | after `N` |
| `- **bold**` | `bo\|ld` | `- bo\n- ld` | after `l` |
| `- **a😀b**` | after `a` | `- a\n- 😀b` | inside the emoji |

A Backspace after the last row leaves U+FFFD where the emoji was. Expected for the first row:
`- \n- [docs](https://example.com) here` with the caret before the link, as `- plain item`
keeps its text and gets its caret at the start of the new item.

## Verification

Mounted `host/web/markup-editor.js` with its wasm in headless Chromium, as
`host/web/tests/markup-editor.test.mjs` does, clicked and pressed Return: the table above. A
temporary test of `exact-markdown-editor` gave the same through `Editor::select` and
`before_input`, and `# li\nnkhttps://examplecom title` for Return in the label of
`# [link](https://example.com) title`; ordered, task, nested and quoted items lose a pair the
same way. A paragraph keeps its pair. Undo restores the source.

## Direction

Split outside the construct when the caret is at its edge, by trying the position before a
hidden opener too. Inside one, close it before the break and reopen it after, so nothing is
orphaned and the selection `edit` returned applies. Until then, carry that selection through
the cleanup's maps (`sim.maps[1..]`) instead of mapping `a + 1`.

## Related

`emptied-bold-line-becomes-rule-or-fence` is another gap in the same cleanup. Not in
`issues/` or `QUEUE.md`.
