# A stream mapper cannot yield, and a data module has no native inflater

**Status:** Closed
**Resolution:** both built: DecompressionStream and CompressionStream (deflate, deflate-raw, gzip) natively in the data module, and an exactStream mapper that may return a promise, its later turns an answer's
**Systems:** the TypeScript socket and stream mappers on Hermes (LLP 1016.000 D7), the 100 ms turn budget (`js/src/lib.rs` `DEFAULT_BUDGET_MS`), the data module's web APIs
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord (`apps/excord/llp/0001-excord.rfc.md` §4.1)
**Date:** 2026-10-23

Status note: an ask

## Symptom

Discord's gateway sends a login's READY as one `zlib-stream` packet: for a large
account, megabytes of compressed JSON. Excord's mapper inflated it with `fflate`
(Hermes has no `DecompressionStream`), parsed it and applied it in one call. On an
M1 Max, a development macOS build against the fixture's padded READY
(`POST /control/ready-size`):

| READY | mapper call | result |
|---|---|---|
| 3 MB (674 KB compressed) | 129 ms | refused: over the 100 ms budget |
| 8 MB (1.8 MB compressed) | 341 ms | refused |

A refused mapper call fails the stream, so the gateway never reaches `ready`. The
reference's repair, `await scheduler.yield()`, is not open to a mapper: its call is
synchronous and returns the snapshot.

## What Excord does now

The mapper queues a packet over 16 KB compressed or 512 KB inflated, and every frame
after one while any wait, and a `digest` mutation drains the queue a bounded step a
turn: 32 KB of compressed input through `fflate`, a megabyte of a scan of the text
for `d.guilds`, the READY parsed without its guilds, or 1.5 MB of its guilds parsed
and applied. Each queued frame keeps its progress, so a digest whose turn the host
refused (its continuation never runs) is taken up by the next one. Its reply carries
a snapshot the view takes when it is newer than the stream's. A 32 MB READY then
lands with no turn over budget. The costs measured on the M1 Max: `fflate` about
40 ms per inflated MB, `JSON.parse` 7.5 ms per MB, the scan 23 ms per MB. The
default 64 MiB heap ran out on a 16 MB READY; Excord's host crates raise it to
256 MiB (`exact_js::Module::set_max_heap`).

## Ask

Either would remove most of the app's apparatus:

- `DecompressionStream("deflate")` in the data module natively (the web has it;
  Excord's would then be one call per packet, at native speed), or
- a mapper that may hand a message to a later turn of its own: an `exactStream`
  mapper returning a promise, given turns with their own budget as an answer's
  are, its snapshot the stream's when it settles.

`JSON.parse` of a whole large READY would still be one native call over 100 ms on
a slow phone; the scan stays the app's.

## Resolution

Both asks are built (Charlie, 2026-11-10).

**Compression and Streams.** The data module on Hermes has WHATWG Streams
(`ReadableStream`, `WritableStream`, `TransformStream`, their readers, writers and
controllers, the two queuing strategies; `js/src/streams.js`) and WHATWG Compression's
`DecompressionStream` and `CompressionStream` for `"deflate"`, `"deflate-raw"` and
`"gzip"`. Each chunk is inflated or deflated by `miniz_oxide` behind the bytes door
(`js/src/compression.rs`, door ops 12 to 15), in the turn that writes it, as
`crypto.subtle` works. Errors are the standard's `TypeError`s. One stream spans many
writes, so Discord's `zlib-stream` inflates packet by packet: after a write settles,
its output is queued. On the web the browser's own are used; the server render's realm
is given Bun's.

**An async mapper.** An `exactStream` mapper may return a promise. Natively the mapper
call is settled as an answer's call is: one that settles in its first turn answers
then; one that yields parks on the stream's key and asks for its next turn as an
answer's continuation, which the runner sends beside the stream's ticket
(`runner/src/runner/mapper_turns.rs`). Each turn has its own budget, and the value is
committed as the stream's when the call gives it. Messages and the stream's end that
land meanwhile wait in order, at most 4096 messages or 64 MiB, and are handed back one
per landing, so a later message's mapper call never overtakes an earlier one's. A
rejection is a throw: the stream ends with that error and the resource keeps its last
value. A stream let go lets its pending call go (`scheduler.yield()` rejects with an
`AbortError`), and its turn's landing is dropped. What the mapper sends before its
first `await` goes with the frame's first commit; a later turn sends as an answer's
turn does. `fetch`, timers and storage are refused in a mapper. The web JS target
chains the mapper's calls the same way (`host/web-js/ts-stream.js`); after the first
`await` it cannot send there. The web's wasm host still maps each message at once and
refuses a promise.

**Excord.** Natively inflating costs about 3 ms per inflated MB (8 MB of JSON from
1.4 MB in about 25 ms, debug build) against `fflate`'s 40 ms, and a mapper can now
spread a READY over turns of its own. That removes `fflate`, the packet queue, the
`digest` mutation and its bounded steps, each queued frame's kept progress, and the
newer-snapshot reconciliation between a digest's reply and the stream's: the mapper
inflates each packet and parses it, awaiting `scheduler.yield()` between slices of a
large one. The scan that splits a large READY so that no single `JSON.parse` is long,
and the raised heap for a 16 MB READY, stay the app's.

## Verification

- `cargo test -p exact-js`: `compression.rs`'s unit tests (deflate, raw and gzip, byte
  by byte; a 20-message sync-flushed zlib stream inflated message by message; corrupt,
  truncated and trailing input, a second gzip member, a bad CRC; 8 MB whole) and
  `tests/it/pure.rs`, where the same bundle (`tests/fixtures/pure-streams.ts`: a
  transform's write, read and close order, piping, teeing, a failing sink, queuing
  strategies, a zlib-stream inflated packet by packet, the three formats, round trips
  and the standard's errors) gives the same output in Hermes and headless Chrome.
- `tests/it/mapper.rs` through the runner: a later message waits for an earlier
  call's yields and the three commit in order; a rejection ends the stream; the
  stream's end waits behind a pending call and its own mapper call awaits; a stream let
  go drops its pending call; replies before the first `await` and after it go out in
  order, and `fetch` and timers are refused; the 4097th held message ends the stream; a
  zlib stream inflates frame by frame.
- Timing (`an_8_mb_frame_inflates_in_one_turn_or_many`, a debug test build on a Mac):
  8 MB of JSON from 1424 KB of deflate in one mapper call, about 27 ms of the turn's
  100 ms budget, within it under the default budget; written in 64 KiB slices with a
  yield between, 23 turns, the longest 1.8 ms.
- `host/web-js/async-mapper.test.mjs`: the web JS target's order, rejection, end and
  synchronous mappers; `host/web-js/socket.test.mjs` and `stream-abort.test.mjs` still
  pass.
