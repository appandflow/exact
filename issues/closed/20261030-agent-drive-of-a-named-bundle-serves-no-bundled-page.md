# A drive of a bundle that `EXACT_MAC_BIN` names serves no bundled page

**Status:** Closed
**Resolution:** The driver reads a named bundle's own Contents/Resources and refuses a missing capture; WebArm logs and shows a src that names no file.
**Systems:** Agent driver (`scripts/agent-stdio.mjs`), Apple host (WebArm)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's files preview lane
**Date:** 2026-10-30

**Problem.** A drive names a bundle with `EXACT_MAC_BIN` from a shell that does not export the `CARGO_TARGET_DIR` the bundle was built with. Then the app serves no bundled page (LLP 1020.001): each such frame is an empty document, with no paint and no `postMessage`, and the WebView node shows `loading=false` and `guest: []`. With the variable exported, the same bundle and ops serve every page. No op reply and no `logs` line says why.

**Cause.** The driver sets `EXACT_ASSETS` to the build capture under its own target directory (`scripts/agent-stdio.mjs:52`, `capture` from `host/apple/build.mjs:188`). It does this even when `EXACT_MAC_BIN` names a binary from somewhere else (`agent-stdio.mjs:26`). WebArm reads a scheme-less `src` as a file under `EXACT_ASSETS`, which takes the place of the bundle's resources (`host/apple/webarm/WebArm.swift:995`). The capture of another target directory has no such file, so `localBytes` returns nil, and `local` is nil (`WebArm.swift:783`). The page is then neither local nor remote, so on macOS it is loaded directly (`WebArm.swift:805`) as a 200 response with an empty body (`WebArm.swift:831`). This path writes nothing to stderr, so the host lines that `logs` returns (`agent-stdio.mjs:75`, `scripts/agent.mjs:814`) have nothing to show.

**Reproduction.** Build an app with a framed page under a different `CARGO_TARGET_DIR`, then run `EXACT_MAC_BIN=<bundle>/Contents/MacOS/<name> bun scripts/agent.mjs macos tree` without that variable: the page's frame is empty.

**Fix.** When `EXACT_MAC_BIN` names a binary inside an `.app`, set `EXACT_ASSETS` to that bundle's `Contents/Resources`. When the capture the driver would use is missing, refuse, as the driver already refuses a stale build. When a scheme-less `src` names no file under the asset root, WebArm writes a line to stderr, as its asset scheme's refusal does (`WebArm.swift:179`), instead of serving an empty document.

**Acceptance.** The reproduction serves the page. A drive of a page whose file is missing shows a line in `logs` that names the file.

## Resolution

The three parts of the fix:

- `scripts/agent-stdio.mjs`: when `EXACT_MAC_BIN` names an executable inside an
  `.app`, `EXACT_ASSETS` is that bundle's `Contents/Resources`, which holds the
  same trees as the capture (`host/apple/bundle.mjs` copies them there).
- The same file refuses a macOS or sample-host drive whose asset root is the
  capture and the capture is missing: for an `EXACT_MAC_BIN` binary it says to
  export the target directory the binary was built with or to set
  `EXACT_ASSETS`; otherwise it names the rebuild. An `EXACT_ASSETS` the caller
  sets is still used as given.
- WebArm (`host/apple/webarm/WebArm.swift` `serve`): a `src` that is neither a
  file under the asset root nor a URL WebKit loads writes
  `exact: iframe <id>: <src> is not a file under <root>` to stderr and shows that
  line as the frame's error document. The host no longer sends an empty
  document for a page it found no file for (`WebModule.swift`): since the
  issue was filed it had sent one, so the page went to the asset scheme, which
  logged a misleading "is not an app asset under assets/ or deck/". The
  asset-root reading moved from `WebArm.swift` to `FrameFiles.swift`, which
  keeps `WebArm.swift` under the 1,500-line cap.

`docs/agent-pitfalls.md` ("`EXACT_MAC_BIN` overrides…") says where each kind of
executable reads its assets.

## Verification

On macOS 27.0.1, with a scratch app (`exact new`) whose `iframe
src="assets/page.html"` posts a message to the app. It was built once into its
own `target/` and once with `CARGO_TARGET_DIR` set to another directory, and the
first build was moved aside so the driver's capture was missing.

- Before (the driver at HEAD), with `EXACT_MAC_BIN` naming the other build's
  bundle: the frame showed no page and the app received no message.
- After: the same drive serves the page (`tree frame` shows its `p#t` "bundled
  page"), and the app's `message` handler runs.
- `EXACT_MAC_BIN` naming the other build's bare executable: the driver refuses
  before launch, naming the missing capture and both remedies.
- `EXACT_ASSETS` pointing at an empty directory: `logs` shows `app: exact:
  iframe 14: assets/page.html is not a file under <that directory>`, and the
  frame shows the same line.
- `bun host/apple/build.mjs --test` (no iframe test failed) and `bun test` for
  `scripts/agent-build`, `agent-launch`, `agent`, `agent-test`, `smoke-inspect`,
  `agent-keys` and `agent-held`. One test there fails, and not because of this
  change: `agent-test.test.mjs` "authored media preferences…" expects an older
  wording of the web driver's `prefer` refusal.
