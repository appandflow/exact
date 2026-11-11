# Recipe: open a page over the app

Install with `bun exact.mjs add @exact/in-app-browser`. In `app.contract`:

```contract
use openBrowser, BrowserResult from "@exact/in-app-browser"

component App
  mutation browser as shape BrowserResult
  action checkout
    send browser = openBrowser(url="https://example.com/checkout")
  view
    column
      button press=checkout testId="checkout"
        text "Check Out"
      text (match browser { case some(r) => `${r.action}`, case none => "" }) testId="result"
```

Pass an absolute HTTP(S) URL. iOS shows `SFSafariViewController` over the app and
answers `{action: "dismissed"}` when it closes. Android opens a Custom Tab (or
falls back to the browser) and answers `{action: "opened"}` on launch. macOS
opens the default browser; the web opens a tab/window; both answer `"opened"`.
Linux and tvOS refuse with `NotSupportedError`. No grant is needed.

**Under the agent nothing opens.** A send waits on a `browser` ticket in
`state.pending`. `clock settle` stops at the hold and prints its target (`@N`);
`state pending` prints the numeric `ticket`. Run this first:

```sh
bun exact.mjs agent web "tap checkout" "clock settle" "state pending"
```

Copy the target it prints into the next operation: `tap @N dismissed` or
`tap @N opened`. In the same drive, answer its first ticket as follows:

```sh
bun exact.mjs agent web "tap checkout" "clock settle" "state pending" "tap @1 dismissed" "clock settle" state logs
```

This command starts a new drive of the example, whose first ticket is `@1`.
The final settle fills the mutation with that action. The web drive's `logs`
records `in-app-browser: dismissed https://example.com` (or `opened`).
A drive proves the request and reply; check the actual browser UI in a normal
app launch: open the page, close it and confirm the app returns to its prior
screen. See the package's `test/checklist.md` for platform checks.
