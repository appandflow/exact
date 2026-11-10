# Independent HTTP refuses or silently holds large ceilings on Apple, though 64 MiB is documented

**Status:** Closed
**Resolution:** A plain independent reply is read into one buffer of its ceiling and charged it once, and the independent lane holds a 64 MiB read beside the largest request; a read that cannot fit beside open streams is refused when made.
**Systems:** the Apple executor (`host/apple/src/executor_core.rs`), `exactIndependentHttp.maxResponseBytes` (`docs/reference.md`)
**Severity:** P2
**Author:** Claude (Opus 5.5), from the T3 Code clone (`examples/t3-code`, PR #1047)
**Date:** 2026-10-26

## Symptom

`docs/reference.md:942` says an independent request's `maxResponseBytes` may be 1 byte to 64 MiB, and
`reservation` checks the same range (`executor_core.rs:1125`). A request with a ceiling above about 32 MiB is still
refused before it starts, with "native response ceiling exceeds the lane's byte budget". The T3 Code clone's PDF
preview asked for 64 MiB, then 50 MiB, and was refused both times; it now asks for 31 MiB (`media-frame.ts`), so a
sent PDF larger than that cannot be previewed.

## Cause

`reservation` charges a job twice its ceiling, for `Vec` growth while collecting (`executor_core.rs:1157`):
`limit = bytes + body * 2 + MAX_HEADERS * 2`. The independent lane's budget is 64 MiB (`BYTES[1]`, `:36`), and a
`limit` over the budget is refused (`:1161`). So the largest ceiling that can run is a little under 32 MiB.

Below that it can still never start. A worker starts a job only once its whole charge is free in the lane, and waits
for it otherwise (`next_job`). Every open stream holds its own charge in the same lane: a socket's 1 MiB ceiling,
charged twice, about 2 MiB. With the T3 Code clone's local server and one paired server connected, a 31 MiB ceiling
(about 62 MiB charged) waited forever with no error: still pending after 40 s, while `curl` fetched the same URL in
4 ms. The clone now asks for 24 MiB (#1049), which fits beside about seven sockets; with more, it would hang again.

## Ask

Make the documented range and the admitted range agree. Either:

- collect into a buffer sized from `Content-Length`, or reserve exactly, so the charge is the ceiling and not twice
  it; or
- raise the independent lane's budget to cover a 64 MiB ceiling; or
- document 32 MiB as the largest ceiling on Apple and check it in `reservation`, so the refusal names the real limit.

Separately, a job whose charge cannot fit beside the lane's long-lived streams should fail (or be bounded by its
timeout) rather than wait with no end.

Other hosts were not checked.

## Partly resolved

A download written to a file (`fetch(url, {exactBodyTo})`, LLP 1108 D6 R2, 2026-10-26) is charged only the 1 MiB
it can hold, whatever its ceiling (up to 1 GiB), so it is neither refused nor left waiting beside open streams. A
preview that reads a file the host downloaded, as the T3 Code clone's PDF preview can, no longer needs a large
ceiling. A reply read into memory still has the 2× charge and the 32 MiB limit, and can still wait with no end.

## Resolution

Of the three options, the exact reservation. `body_to::receive` reads a plain independent reply into one buffer of
its ceiling, reserved before the first read and never grown, then trimmed to its length; one byte over is refused as
before. `reservation` therefore charges such a reply its ceiling once (`reads_within_ceiling`). Streams, processes,
native calls and the ordered lane keep the doubled charge, because their outcomes still grow as they are collected.
The Darwin transport makes no body-sized allocation of its own (`DarwinTransport`'s doc), so this buffer is the
reply's only copy. The asynchronous core (off by default) grows its buffer only up to the ceiling, so its final
capacity is also within the single charge.

A 64 MiB ceiling also needs the request buffers and the header allowance beside it, so the independent lane's budget
is now that largest job: `MAX_REQUEST + MAX_BODY + 2 × MAX_HEADERS`, 68 MiB and 128 KiB, up from 64 MiB. Each read
now costs half what it did, so the lane holds about twice the reads it did.

The wait-forever half was already settled for the reported order: a read whose ceiling does not fit beside the open
streams is refused when it is made ("exceeds the lane budget held by open streams"). A stream opened after a large
read is admitted can still keep that read waiting until a stream ends or its `exactTimeout` passes; that is in
`QUEUE.md`.

Every native host compiles `executor_core.rs` (Linux, the render host, and Windows and Android through
`exact-linux`), so the fix covers them. The web hosts have no lane budget and already admitted 1 byte to 64 MiB.

`docs/reference.md` states the new budget and the streams rule. LLP 1041 §8.4 ("64 MiB", "twice its ceiling while
running") and LLP 1054.000 R3's limit ("about 32 MiB") describe the old charge and need amending.

## Verification, 2026-11-10

The three new tests in `executor_body_to_tests.rs` failed before the change with "native response ceiling exceeds
the lane's byte budget" and pass after it: a 64 MiB ceiling's reservation is the ceiling plus the header allowance
and fits the lane beside 4 MiB of request buffers; a 40 MiB reply under a 64 MiB ceiling, and a 100,000-byte reply
under a 100,000-byte ceiling, arrive intact with capacity equal to length, and 100,001 bytes is refused; with an
8 MiB stream open, a 64 MiB read is refused when made and a 48 MiB read answers. Tests tuned to the doubled charge
and the 64 MiB lane were retuned to the new numbers. `cargo test -p exact-apple --lib` passes (585), including the
suite's rerun on the asynchronous core; clippy and fmt are clean. The Cargo suites `scripts/affected.mjs` names pass
but for three integration tests (`exact-js-bake` producer, `fieldnotes-apple` notebook, `weatherlight-apple` weather)
that fail identically with the executor sources before this change.
