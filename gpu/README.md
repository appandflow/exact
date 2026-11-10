# GPU module ABI

The host and module ship together. Before 1.0, missing required symbols refuse
loading; there is no compatibility shim. See LLP 1009 and LLP 1014.000.

| Version / revision | Child delivery export | Contract |
|---|---|---|
| GPU ABI 1 / exact-gpu 0.1.0, before `126daba5` | `gpu_child` | Unnamed child pixels and geometry. |
| GPU ABI 1 / exact-gpu 0.1.0, from `126daba5` | `gpu_child_view` | **Breaking host ABI change:** adds UTF-8 `testId` and its byte length before geometry and pixel arguments, on native and Wasm. All embedders must rebuild against this revision. |

GPU tests use `exact_gpu::fixture::device_or_skip`: only `no adapter:` skips;
request-device, configuration, validation, and device-loss errors fail.

Windows rendered modules also export `gpu_window_frame` and `gpu_window_detach`;
rebuild the host and module together. A full-window canvas can present directly
from its module-owned GPU, composing premultiplied RGBA UI layers below and above
the game. The HWND and HINSTANCE remain valid until detach or module unload.
No GPU device or texture object crosses the DLL boundary. Transformed, clipped,
multiple-canvas and backdrop-dependent layouts use ordinary capture instead.
Canvas ID zero presents the complete CPU UI frame after the game is unmounted,
so menus continue through the same swapchain instead of switching back to GDI.
The final flags argument uses bit 0 for changed UI and bit 1 for a solid underlay.
With bit 1 set, the below pointer holds only four premultiplied RGBA bytes;
otherwise it holds the full pixel buffer. The above pointer always holds `len`
bytes. A solid underlay is cleared on the GPU without a CPU pixel upload.
The final two arguments, `above_top` and `above_bottom`, bound all nontransparent
overlay pixels in device rows (end exclusive). Unknown bounds use the whole
height. The uploader compares the union of current and previous bounds, so
moving or removing UI also clears its old pixels. Rebuild both host and module
for this signature.
Capture reuses its render target and staging buffer until size, format or device
changes. The Windows host follows the current monitor's refresh rate.

For native performance measurements, set `EXACT_AGENT_WINDOW_FRAMES` (1–10000)
and optionally `EXACT_AGENT_WINDOW_TAP` to a launch button's test ID.
`EXACT_AGENT_WINDOW_FULLSCREEN=1` uses borderless fullscreen;
`EXACT_AGENT_WINDOW_UNPACED=1` removes pacing to measure rendering capacity.
These switches require a bounded development run and are stripped in production.

## A module's own audio output

A surface posts the string `"exact:audio"` through `Surface::messages` to ask
for the app's audio session. The host consumes it without dispatching it to
the app. On iOS and tvOS, ExactKit applies the app's `audio_session` category
and options and activates the session. A first successful request sends
nothing back. A failed activation sends `Lifecycle::Interrupted` to the
host's surfaces; recovery sends `Lifecycle::Resumed`. Failure retries after
300 live display-link frames, on a Hidden → Visible transition or a key/pointer
gesture. An interruption ending without `shouldResume` blocks automatic
retries, including repeated `exact:audio` requests, until visibility or input
allows recovery. Hidden surfaces do not retry.

Immediately before a GPU module makes a surface on iOS or tvOS, the owner
sets an unclaimed session to `.ambient`, without activating it. Output opened
before its request is handled therefore mixes with other apps' audio and
obeys the ring/silent switch until the request takes the app's configuration.
A held or claimed session stays as it is. This preparation does not run for
Canvas 2D, an app with no GPU canvas, or the driver's frozen clock, which
makes surfaces seekable and opens no output. Under `EXACT_AGENT_TIMING=platform`
the owner prepares too: surfaces can open output, but the driver drops
`exact:audio` requests, so they do not raise the prepared ambient session.
macOS, the web and Linux consume the request without configuring a session.
