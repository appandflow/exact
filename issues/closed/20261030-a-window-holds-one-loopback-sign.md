# A window holds one loopback sign-in, and a callback without a code ends it

**Status:** Closed
**Resolution:** already fixed on this checkout: loopback sign-ins wait side by side, one per address, and the receiver and the agent take only a callback with one code or one error (LLP 1133.005 D9, as amended 2026-10-30)
**Systems:** runner auth (`runner/src/auth.rs`), the loopback receiver (`host/listen/src/loopback.rs`), LLP 1069.006 D3, LLP 1133.005 D9
**Author:** Claude (Opus 5.5), from the T3 Code clone
**Date:** 2026-10-30

**Problem.** T3 Code signs in two ChatGPT accounts at once, each on its own loopback port, and both complete
(`apps/desktop/src/app/CodexAuthCallback.test.ts`, "allows two accounts to complete independently"). In Exact the
second `openAuthSession` is refused 409 `already open`. T3 answers a callback with the right `state` but no `code`
or `error` 400 and keeps listening (`codexCallbackUrl`, `packages/shared/src/codexAuthHandoff.ts`); Exact's
receiver answers it 200 and stops, and the sign-in must start again.

**Cause.** `arm` refuses any session while another target's is live (`runner/src/auth.rs:673-680`, LLP 1069.006 D3,
made for a sheet). The receiver accepts on `state` alone (`host/listen/src/loopback.rs:215-246`) and stops (`:201-205`).

**Reproduction.** Two targets' loopback sign-ins under the agent: the second is 409.

**Fix.** A loopback sign-in shows no sheet: it neither refuses nor is refused by another target's session, and a
second one on a live one's host and port is 502 `port in use`, as its bind is. Its callback must carry exactly one
non-empty `code` and no `error`, or the reverse (RFC 6749 §4.1.2, §4.1.2.1); else the receiver answers 400 and keeps
listening, and the agent's `type @t` is refused before the hold is spent. The receiver drops a fragment before it
reads the query, as the runner does.

**Acceptance.** `cargo test -p contract --test it auth`, `-p exact-listen -- --include-ignored`; on macOS,
`smoke-auth.mjs` and an `EXACT_DEVICE=real` drive: two sign-ins at once, a stray callback 400.

## Resolution

No change was needed: the fix landed with LLP 1133.005 D9's amendment of 2026-10-30, and only this
file stayed open. `arm` (`runner/src/auth.rs`) refuses 409 only between sheets and popups; a
loopback sign-in on an address a live one holds is 502 `port in use`. `Live::accept` adds
`one_code_or_error` for a loopback callback, so the agent's `type @t` is refused before the hold
is spent. The receiver (`host/listen/src/loopback.rs`) drops a fragment, answers 400 to a request
without exactly one non-empty `code` or `error`, and keeps listening.

## Verification (2026-11-10)

- `cargo test -p contract --test it auth`: 54 passed, among them
  `loopback_sessions_wait_side_by_side_and_take_only_a_code_or_an_error`.
- `cargo test -p exact-listen -- --include-ignored`: every loopback test passes, among them
  `two_receivers_on_two_ports_complete_independently` and
  `a_fragment_is_dropped_as_the_runner_drops_it`. The unix-socket test
  `a_connection_can_be_answered_before_its_callback_returns` fails under macOS's per-user
  `TMPDIR`, whose socket path reaches `sun_path`'s limit; with a short `TMPDIR` all 21 pass.
- `bun scripts/smoke.mjs macos --app auth-fixture --app-only`: the auth drive passes 18 of 18,
  two loopback holds at once and a callback without a code refused among them.
- `EXACT_DEVICE=real` on macOS: with the fixture's approval left to a person, both receivers
  listened at once on 4332 and 4333; a request with no query, a wrong `state`, or a wrong
  `state` and a `code` was answered 400 on each, both kept listening, and quitting freed both
  ports. With `/fixture/auto?on=1`, the default browser returned to both receivers and both
  sign-ins completed (`auth 14` and `auth 15: callback accepted`, two token exchanges, PKCE and
  DPoP checked).
