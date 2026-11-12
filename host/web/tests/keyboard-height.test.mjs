// LLP 1039 D9: the JS target answers exactViewport().keyboardHeight as the
// page reads the keys, under every interactive-widget policy.
import { test, expect } from 'bun:test';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../../..', import.meta.url)));

async function served(contract, drive, options = {}, init = null) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-keyboard-height-'));
  let server, browser;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Keyboard height', app: { id: 'com.example.keyboardheight', name: 'Keyboard height' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), contract);
    const dist = resolve(dir, 'dist');
    const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'keyboard-height', '--out', dist, '--render', 'js'], {
      cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir },
    });
    expect(build.status, build.stderr + build.stdout).toBe(0);
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: req => {
      const path = new URL(req.url).pathname;
      if (['/navigation.js', '/grant-admission.js'].includes(path)) return new Response(Bun.file(resolve(dist, '.gen', path.slice(1))));
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await suiteBrowser();
    const page = await browser.newPage(options);
    if (init) await page.addInitScript(init);
    await page.goto(`http://127.0.0.1:${server.port}/?agent`);
    await page.waitForFunction(() => typeof globalThis.exact?.agentSettled === 'function');
    const resource = () => page.evaluate(async () => (await exact.agentSettled({ op: 'state' })).resources.viewport);
    // Polled from here: a page function's promise is truthy before it settles.
    const until = async (holds, what) => {
      let v;
      for (let i = 0; i < 200; i++) {
        v = await resource();
        if (holds(v)) return v;
        await Bun.sleep(50);
      }
      throw new Error(`${what}: the resource is ${JSON.stringify(v)}`);
    };
    await drive({ page, resource, until, cdp: await page.context().newCDPSession(page) });
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}

// Desktop Chrome draws no software keyboard: the visual viewport stands in
// for one, shrinking from the bottom as a phone's keys make it, and then the
// VirtualKeyboard API's rect, which wins where the page overlays the keys.
test('keyboardHeight is what the keyboard covers', () => served([
  'shape Viewport',
  '  keyboardHeight: number',
  'component App',
  '  resource viewport = exactViewport() as shape Viewport',
  '  view',
  '    text `${viewport.keyboardHeight}` testId="keyboard"',
  '',
].join('\n'), async ({ page, resource, until }) => {
  expect((await resource()).keyboardHeight).toBe(0);
  const keys = height => page.evaluate(height => {
    window.keys = height;
    visualViewport.dispatchEvent(new Event('resize'));
  }, height);
  await keys(291);
  await until(v => v.keyboardHeight === 291, 'the visual viewport\'s keyboard');
  await page.waitForFunction(() => document.querySelector('[data-testid="keyboard"]')?.textContent === '291');
  expect(await page.evaluate(async () => (await exact.agentSettled({ op: 'layout' })).env['keyboard-inset-height'])).toBe(291);
  await keys(0);
  await until(v => v.keyboardHeight === 0, 'the keys gone');
  await page.evaluate(() => { window.keysApi = 336; navigator.virtualKeyboard.dispatchEvent(new Event('geometrychange')); });
  await until(v => v.keyboardHeight === 336, 'the VirtualKeyboard API\'s rect');
}, {}, () => {
  const height = Object.getOwnPropertyDescriptor(VisualViewport.prototype, 'height').get;
  Object.defineProperty(VisualViewport.prototype, 'height', { configurable: true, get() { return height.call(this) - (window.keys ?? 0); } });
  if (navigator.virtualKeyboard) {
    Object.defineProperty(navigator.virtualKeyboard, 'boundingRect', { configurable: true, get: () => new DOMRect(0, innerHeight - (window.keysApi ?? 0), innerWidth, window.keysApi ?? 0) });
  }
}), 180000);

// Under `interactive-widget=resizes-content` the keys shrink the layout
// viewport as well as the visual one: on a phone (a coarse pointer) the
// overlap is measured against the height the page had before them, in its
// orientation, and carried through the screen when it turns with the keys up.
// A window resized on a desktop is a window, focused field or not.
const FIELD = [
  'shape Viewport',
  '  keyboardHeight: number',
  '  width: number',
  '  height: number',
  'component App',
  '  resource viewport = exactViewport() as shape Viewport',
  '  state name = ""',
  '  action edit(v: string)',
  '    name = v',
  '  view',
  '    column interactive-widget="resizes-content"',
  '      input value=name input=edit aria-label="Name" testId="name"',
  '      text `${viewport.keyboardHeight}` testId="keyboard"',
  '',
].join('\n');
// The window's size (`window.size`, else the browser's) less the keys, which
// both viewports lose, as `resizes-content` has it.
const viewportsUnderKeys = () => {
  const width = Object.getOwnPropertyDescriptor(window, 'innerWidth').get;
  const height = Object.getOwnPropertyDescriptor(window, 'innerHeight').get;
  Object.defineProperty(window, 'innerWidth', { configurable: true, get() { return window.size?.[0] ?? width.call(this); } });
  Object.defineProperty(window, 'innerHeight', { configurable: true, get() { return (window.size?.[1] ?? height.call(this)) - (window.keys ?? 0); } });
  Object.defineProperty(VisualViewport.prototype, 'height', { configurable: true, get() { return innerHeight; } });
};
const turn = (page, size, keys) => page.evaluate(([size, keys]) => {
  window.size = size;
  window.keys = keys;
  dispatchEvent(new Event('resize'));
  visualViewport.dispatchEvent(new Event('resize'));
}, [size, keys]);
const PHONE = { hasTouch: true, viewport: { width: 390, height: 844 }, screen: { width: 390, height: 844 } };

test('keyboardHeight on a phone under resizes-content is the keys, in either orientation', () => served(FIELD, async ({ page, resource, until }) => {
  expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
  await page.focus('[data-testid="name"]');
  await turn(page, [390, 844], 300);
  let v = await until(v => v.keyboardHeight === 300, 'the keys under resizes-content');
  expect(v.height).toBe(544);
  await page.waitForFunction(() => document.querySelector('[data-testid="keyboard"]')?.textContent === '300');
  // Turned with the keys up: a landscape page 390 tall, 200 of it keys.
  await turn(page, [844, 390], 200);
  v = await until(v => v.width === 844, 'the turn');
  expect(v.keyboardHeight).toBe(200);
  expect(v.height).toBe(190);
  // Back, the keys gone: the portrait page again, no keys.
  await turn(page, [390, 844], 0);
  await until(v => v.keyboardHeight === 0 && v.height === 844, 'the keys gone');
  // Nothing typed into: a shorter page is not keys.
  await page.evaluate(() => document.activeElement.blur());
  await turn(page, [390, 844], 120);
  await until(v => v.height === 724, 'the shorter page');
  expect((await resource()).keyboardHeight).toBe(0);
}, PHONE, viewportsUnderKeys), 180000);

test('keyboardHeight on a desktop is 0 when the window shrinks with a field focused', () => served(FIELD, async ({ page, resource, until }) => {
  expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(false);
  const full = await page.evaluate(() => innerHeight);
  const width = await page.evaluate(() => innerWidth);
  await page.focus('[data-testid="name"]');
  await turn(page, [width, full], 120);
  await until(v => v.height === full - 120, 'the shorter window');
  expect((await resource()).keyboardHeight).toBe(0);
}, {}, viewportsUnderKeys), 180000);
