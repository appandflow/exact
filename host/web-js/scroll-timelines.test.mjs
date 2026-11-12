// LLP 1057.003 D5 on the JS target: the timelines fixture (host/web/tests/fixtures/timelines.contract) built by this
// target runs every scroll timeline as the browser's own, and each consumer shows what the browser recorded
// for the wasm target's build of the same app (browser-timelines.json, `parity.mjs --timelines`), which the engine is
// held to as well. Without the fixture's drag timeline the plan has none, so every name is CSS's own row and no motion
// piece is needed; the recording itself is of the fixture whole, where the motion piece hands each name that finds a
// scroll timeline to the browser (motion-glue.js `followTimelines`). Build exact-web-js once before running
// this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { withLook } from '../web/look.mjs';

test('scroll timelines in a plan with no drag timeline show what the browser recorded', async () => {
  const root = resolve(import.meta.dir, '../..'), fixtures = resolve(root, 'host/web/tests/fixtures');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const dir = mkdtempSync(resolve(target, 'scroll-timelines-'));
  const style = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
  const steps = JSON.parse(readFileSync(resolve(fixtures, 'timelines-steps.json'), 'utf8'));
  const recorded = JSON.parse(readFileSync(resolve(fixtures, 'browser-timelines.json'), 'utf8')).samples;
  let server, browser;
  try {
    const source = readFileSync(resolve(fixtures, 'timelines.contract'), 'utf8').replace(/^.*-exact-drag-timeline.*\n/m, '');
    expect(source).not.toContain('-exact-drag-timeline');
    writeFileSync(resolve(dir, 'app.contract'), source);
    const compiled = spawnSync(resolve(target, 'debug/exact-web-js'), ['js', resolve(dir, 'app.contract'), '-o', dir], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${style}</style><link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
    const page = await (await browser.newContext({ viewport: { width: 420, height: 900 } })).newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('[data-testid="sv"]'));
    const wrong = [];
    for (const step of steps) {
      for (const at of step.at) {
        const got = await page.evaluate(async ({ step, at }) => {
          const el = document.querySelector(`[data-testid="${step.scroller}"]`);
          el[step.axis === 'x' ? 'scrollLeft' : 'scrollTop'] = at;
          await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
          return Object.fromEntries(step.read.map((id) => [id, +getComputedStyle(document.querySelector(`[data-testid="${id}"]`)).opacity]));
        }, { step, at });
        const want = recorded.find((s) => s.scroller === step.scroller && s.at === at)?.values ?? {};
        for (const id of step.read) {
          if (!(Math.abs(got[id] - want[id]) <= 1e-3)) wrong.push(`${step.scroller}@${at} ${id}: ${got[id]}, recorded ${want[id]}`);
        }
      }
    }
    expect(wrong).toEqual([]);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
