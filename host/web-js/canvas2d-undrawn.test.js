// A 2D canvas on the JS target whose Rust module turns out to draw no Canvas 2D surface: the build sent it there because
// no GPU module registers its surface, so nothing will draw it. The journal and the canvas's state say so, by name,
// whichever comes first: the module or the canvas.
import { test, expect, mock } from "bun:test";

mock.module("./canvas2d-glue.js", () => ({}));
mock.module("./draw.js", () => ({ drawer: null }));

test("a canvas nothing draws says so in its state and the journal", async () => {
  const kept = ["getComputedStyle", "matchMedia", "MutationObserver", "document", "exact"].map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  try {
    globalThis.getComputedStyle = () => ({ color: "rgb(0, 0, 0)", direction: "ltr" });
    globalThis.document = { documentElement: {}, addEventListener() {} };
    globalThis.exact = { canvas2dGlue: () => ({ op() {} }) };
    const { engine } = await import("./canvas2d.js");
    const said = "no GPU module registers this surface, and the Rust module draws no Canvas 2D surface (exact_logic_abi::export_draw!)";
    const canvas = (name) => ({ e: { isConnected: true, dataset: {} }, name, args: [], types: [], names: [], mounted: 0 });
    // As a page usually runs: the Rust module, with no draw, loaded before the Canvas 2D chunk arrives and its first canvas
    // mounts. Each surface is named once.
    {
      const journal = [];
      const canvases = engine({ data: { logic: { exports: {} }, ready() {} }, clock: { now: 0 }, inflight: { n: 0 }, journal, After: [], views: new Map(), viewId: (() => { let n = 1; return () => n++; })() });
      canvases.args(canvas("glass"));
      canvases.args(canvas("glass"));
      canvases.args(canvas("ink"));
      expect(globalThis.exact.canvas2dState().map((c) => [c.surface, c.error])).toEqual([["glass", said], ["glass", said], ["ink", said]]);
      expect(journal).toEqual([`t=0 canvas2d: nothing draws glass: ${said}`, `t=0 canvas2d: nothing draws ink: ${said}`]);
    }
    // A canvas that mounts before the module loads.
    {
      let loaded = null;
      const journal = [], data = { logic: null, ready: (f) => { loaded = f; } };
      const canvases = engine({ data, clock: { now: 0 }, inflight: { n: 0 }, journal, After: [], views: new Map(), viewId: () => 7 });
      canvases.args(canvas("glass"));
      expect(globalThis.exact.canvas2dState()[0].error).toBe(null);
      data.logic = { exports: {} };
      loaded();
      expect(globalThis.exact.canvas2dState()[0]).toMatchObject({ surface: "glass", draws: 0, error: said });
      expect(journal).toEqual([`t=0 canvas2d: nothing draws glass: ${said}`]);
    }
  } finally { for (const [name, d] of kept) d ? Object.defineProperty(globalThis, name, d) : delete globalThis[name]; }
});
