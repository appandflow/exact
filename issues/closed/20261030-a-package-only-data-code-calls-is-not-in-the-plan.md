# A package that only data code calls is not in the plan

**Status:** Closed
**Resolution:** the compiler reads the packages TypeScript reached from app.ts imports and puts each module declaration among them in the plan, as a Contract use of a name would
**Systems:** Contract lowering (used modules), JS bake (`usedPackages`), agent permissions
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's adopt-native lane
**Date:** 2026-10-30
**Related:** LLP 1133.004, LLP 1133.011 D12

**Problem.** The T3 Code clone posts notifications from data code (`src/shell/shell-notify.ts`, `src/pull-requests/pages-pr-guide.ts`). No Contract file reads a name of `@exact/notifications`. Without such a read, the package has no rows in the plan. Under the agent, `--permission notifications=granted` is refused with "prefer permission: no module here asks for `notifications`", and data code's calls are refused with `NotSupportedError`, so nothing posts. Outside the agent, the calls are linked and work, so the gap shows only in drives and tests. A `use NotificationStatus from "@exact/notifications"` (a shape) does not help. The clone keeps a `notificationStatus()` read in `app.contract` only to put the package in the plan.

**Cause.** The lowering puts a module in the plan only when a Contract source reads one of its names, the root hears its event, or it has a name that only data code calls (`"contract": false`): `contract/lower/src/lib.rs:588`. Every `@exact/notifications` name is also a Contract name, so a use from data code alone matches none of the three. The bake finds the import (`usedPackages`, `js/bake/src/typescript.mjs:1169`) but uses it only to link the implementation. LLP 1133.004 "As built" records the behaviour (`llp/1133.004-notifications.rfc.md:677`). The refusal is at `runner/src/runner/modules/keyed.rs:325`.

**Reproduction.** An app whose TypeScript data code calls `@exact/notifications` and whose Contract reads none of its names: `bun scripts/agent.mjs macos --permission notifications=granted tree` is refused.

**Fix.** A package that data code imports enters the plan as a used module, as one that Contract reads does. Either the bake's `usedPackages` result, or a `use` of the package in any Contract file, marks the module used in the lowering.

**Acceptance.** The reproduction grants the permission, and the data code's notification posts under the agent. The clone drops its `notificationStatus()` read.

## Resolution

When the program's root is an app's `app.contract`, the loader reads the packages the app's
TypeScript imports (`contract/cli/src/sources/data.rs`): from `app.ts` through relative
imports, every bare specifier's package, leaving out `exact:`, `node:` and type-only imports.
The walk is the TypeScript half of the bake's `usedPackages`, so the plan names no package
the bake does not link. Each such package whose entry is a module declaration (`exact.json`)
is loaded as a `use` would load it, and its names are marked `imported`
(`ModuleDecl::imported`). The lowering counts an imported module as used
(`contract/lower/src/lib.rs`), so its rows are in the plan whatever Contract reads. A
package that is not a module declaration (`@exact/oauth`, a component library) is left to
the bake as before.

Rust data code is not read: a Rust data crate's call of a package that no Contract file
names still has no rows. `docs/agent-pitfalls.md`'s two entries on `@exact/browser` and
`@exact/browser-automation` now say so.

The in-repo apps' plans are unchanged: each already names the packages its TypeScript
imports. The T3 Code clone is outside this repository; its `notificationStatus()` read can
now be dropped there.

LLP 1133.004's "As built" note ("A module imported only from data code is linked … unless
the app's Contract also names the module") no longer describes the build.

## Verification

`a_package_only_data_code_imports_is_in_the_plan` (`contract/cli/tests/it/notify.rs`)
compiles an app whose Contract names nothing of `@exact/notifications` and whose `app.ts`
imports `./src/shell/shell-notify.js`, which imports the package. Without that file the plan
has no module rows; with it the plan has the package's rows, `prefer permission
notifications granted` is accepted, and data code's `showNotification` is listed by the
substitute. `sources::data::tests` checks the walk's reading of import forms and comments.

| Command | Result |
|---|---|
| `cargo test -p contract --test it notify::a_package_only` before the fix | Fails: the plan has no `notifications` rows. |
| `cargo test -p contract --test it notify::` after | 6 passed. |
| `cargo test -p contract -p contract-lower -p contract-syntax -p contract-types --tests --no-fail-fast` | Pass (2,614 in `it`). |
| `cargo run -p contract-difftest -- quick` | Agreed. |
| `cargo test -p exact-js-bake --tests --no-fail-fast` | 2 failures, both environmental and in fixtures that import no package: `snapback4/ts` is absent, and a child process's shared secrets are unconfigured. |
| The in-repo apps whose TypeScript imports an `@exact` package, plus Caltrain and Messages, compiled with and without the new rule (a temporary switch, not kept) | Identical module rows. |
| `bun exact.mjs agent web --permission notifications=granted tree "tap post-data" "clock settle" state logs` on a scratch app made by `exact new` (Contract reads nothing of the package; data code posts from `src/shell/shell-notify.ts`) | The permission reads granted, the post is listed (`Thread 3 is done`, silent, `/thread/3`), and the mutation answers `posted`. |
| The same on macOS | Not run: this worktree's Hermes bundle installer fails, so no macOS TypeScript build can be made here. |
| `cargo clippy -p contract -p contract-lower -p contract-syntax --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `bun scripts/caps.mjs` | Pass. |
