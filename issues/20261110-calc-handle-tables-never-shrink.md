# The `calc()` handle tables keep every distinct value for the life of the process, and two are searched from the start on each lowering

**Status:** Open
**Systems:** kernel/src/style/handles.rs, kernel/src/style/calc/mod.rs
**Severity:** P3
**Author:** Claude (Fable 5.2), from the 2026-11-10 project review
**Date:** 2026-11-10

## What is wrong

Line references are at commit `dd6f5dcd`.

A length that holds a percentage beside something else is a handle into one of three
process-wide tables:

- `calc::TABLE` (`kernel/src/style/calc/mod.rs:301`): every calculation tree and every
  `env()`-fallback text, interned when the value is set;
- `CALC_PAIRS` (`kernel/src/style/handles.rs:12`): one entry per distinct
  `calc(<p>% + <n>px)` lowered to the layout engine;
- `TREES` (`handles.rs:66`): one per distinct tree, floor and added length lowered.

Nothing removes an entry. `style::wide` reclaims its colour table by mark and sweep
(`kernel/src/style/wide/reclaim.rs`); these three have no counterpart.

`intern_pair` (`handles.rs:55`) and `tree_handle` (`handles.rs:84`) also find an entry
with `iter().position(..)` under the mutex, so lowering one `calc()` length costs the
number of distinct values the process has ever lowered.

## The failure

A length bound to a value that keeps changing mints an entry per value:

```
view left=`calc(${played / duration * 100}% - 6px)`   // a scrubber's thumb
```

Measured in a `host-dev` build, each style set and lowered once:

```
 2,000 distinct values:  2.9 ms      16,000: 149 ms
 4,000:                  8.9 ms      32,000: 593 ms
one more lowering after 62,000 trees: 27 µs; after 56,000 pairs: 15 µs; with one entry: 0.2 µs
200,000 distinct trees set, none lowered: 40 MB resident, 207 bytes each
```

Doubling the count quadruples the time. A binding that changes every frame at 60 Hz
mints 216,000 values an hour: about 45 MB an hour, and each lowering then costs about
0.1 ms. Expected: lowering a value costs the same however many values came before,
and the memory of a value no node holds is returned.

No app in the repository binds such a value per frame. `apps/spark/app.contract:228`
(`calc(50% - ${cardWidth / 2}px)`) and `examples/calendar/src/month.contract:56` mint
a handful, and ten sweeps of a drag over 600 half-pixel positions cost 6 ms in all.
The growth matters to an app that runs for hours with a `calc()` bound to playback, a
clock or a sensor.

## Verification

Two temporary kernel tests (`StyleProps::set_dynamic` on `left`, then
`StyleProps::to_taffy`) under `cargo test --profile host-dev` printed the numbers
above; the resident size is `ps -o rss` before and after. A debug build is about
seven times slower (32,000 values: 4.0 s). Not measured: a running app.

## Direction

Look a pair or a tree up by hash, as `calc::TABLE` already does. Reclaim entries with
the styles that hold them, as `wide/reclaim.rs` does for wide colours, or keep the
engine's handles per kernel so they go with it.

## Related

Nothing in `issues/`, `QUEUE.md` or `docs/agent-pitfalls.md` names `CALC_PAIRS`,
`tree_handle` or `Comparison::intern`.
