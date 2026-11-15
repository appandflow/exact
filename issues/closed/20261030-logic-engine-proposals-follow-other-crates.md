# exact-logic's wasm engine accepts SIMD when another crate of the app enables wasmi's `simd`

**Status:** Closed
**Resolution:** load validates against an explicit proposal set before wasmi, so another crate's simd or memory64 feature no longer widens it
**Systems:** Logic (Rust wasm executor, `logic/src/wasm.rs`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's timeline lane
**Date:** 2026-10-30

**Problem.** exact-logic depends on wasmi without the `simd` feature (`logic/Cargo.toml:16`), so built alone its engine refuses a Rust logic module that uses SIMD. When another crate in the same app enables that feature, Cargo unifies features, and exact-logic's engine then accepts SIMD and relaxed-SIMD modules. Nothing reports the change. The T3 Code clone (`daehyeon/t3-code-main`) does this: its data crate runs `@pierre/highlights`' lexers with wasmi and `simd` (`examples/t3-code/data/Cargo.toml:26`), and `t3-code-macos` links that crate and exact-logic together.

**Cause.** `logic/src/wasm.rs:39` starts from `Config::default()` and sets fuel, the start function, multi-memory and stack limits, but no SIMD proposal. wasmi 2.0's default features set SIMD and relaxed SIMD to `cfg!(feature = "simd")` (`wasmi-2.0.0/src/engine/config.rs:94-95`), so they are on whenever any crate compiles the feature in. `Config::wasm_simd` and `wasm_relaxed_simd` exist only with that feature (`config.rs:323,334`), so exact-logic cannot turn them off while it leaves the feature off. `memory64` (`config.rs:92`) follows its feature the same way.

**Reproduction.** Add a data crate that depends on wasmi with `features = ["simd"]` to an app with a Rust logic module, and give that module a SIMD instruction: `load` accepts the module. Without the data crate, `load` refuses it.

**Fix.** exact-logic sets each proposal it accepts explicitly, so another crate's features cannot widen them. For example, depend on wasmi with `simd` and call `.wasm_simd(false).wasm_relaxed_simd(false)` (and the same for `memory64`). Or check the module's features against an allow-list before `Module::new`.

**Acceptance.** A test in `logic` loads a module with one SIMD instruction and gets a refusal, also when the test crate enables wasmi's `simd` feature.

## Resolution

`load` (`logic/src/wasm.rs`) first validates the module with wasmparser against
`PROPOSALS`, an explicit set: wasmi's defaults without multi-memory, SIMD, relaxed SIMD and
memory64. Only then does it call `Module::new`, whose wasmi validation may accept more. The
set does not depend on wasmi's crate features, so another crate of the app cannot widen it.
The validator is wasmparser 0.228, the version wasmi 2.0 already links, so no second copy
is linked; `Cargo.lock` gains only the edge.

The issue's other fix, enabling wasmi's `simd` and `memory64` and turning them off in
`Config`, was not taken: compiling `simd` into wasmi adds about 400 KB to a stripped,
size-optimized binary that links it (1,150,080 to 1,549,328 bytes in a probe of wasmi
alone). The cost of the fix taken is a second validation pass when a module loads, after
first pixel: 1.4 ms for Caltrain's 358 KB module on an M-series Mac, beside wasmi's 1.6 ms.

## Verification

`wasm_refuses_simd_and_memory64_whatever_wasmi_compiles_in` (`logic/src/tests.rs`) loads a
module with one `i32x4.splat` and one with an `i64` memory and expects refusals. exact-logic's
dev-dependencies compile wasmi's `simd` and `memory64` in, so the test runs with both on.

| Command | Result |
|---|---|
| `cargo test -p exact-logic --lib wasm_refuses` before the fix | Fails: `load` accepts the SIMD module. |
| The same after the fix | Passes. |
| The same with the dev-dependency removed (wasmi without `simd`) | Passes. |
| `cargo test -p exact-logic -p exact-linux-update -p exact-apple-update -p caltrain-logic --tests --no-fail-fast` | Pass. |
| Caltrain's release `caltrain_logic.wasm` through `load` (a test not kept) | Loads. |
| `cargo clippy -p exact-logic --all-targets -- -D warnings`, `cargo fmt --all -- --check` | Pass. |
| `bun scripts/caps.mjs` | Pass, wasm32 check included. |
