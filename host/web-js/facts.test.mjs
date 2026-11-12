import { expect, test } from 'bun:test';
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { bundleHostModule } from './glue.mjs';

// LLP 1139.003 D2: a plan that reads only `exactViewport()` shares the
// session's one page reporter, so the agent's `prefer gpu-tier` reaches its
// `gpuTier`; with no prefer it is the agent's launch value, `high`.
test('a viewport-only plan reads the tier the agent prefers', async () => {
  const media = new Map();
  Object.assign(globalThis, {
    location: { search: '?agent' },
    document: { createElement: () => ({}), addEventListener() {}, documentElement: { style: { setProperty() {} } } },
    addEventListener() {}, matchMedia: query => ({ matches: false,
      addEventListener: (_, f) => media.set(query, f), removeEventListener: () => media.delete(query) }),
    innerWidth: 390, innerHeight: 844, devicePixelRatio: 2,
  });
  // As the build lays them out: the web host's navigation.js beside facts.js; the runtime stubbed to its data seam.
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-facts-'));
  try {
    copyFileSync(resolve(import.meta.dir, 'facts.js'), resolve(dir, 'facts.js'));
    writeFileSync(resolve(dir, 'navigation.js'), await bundleHostModule(resolve(import.meta.dir, '../..'), 'navigation.js'));
    writeFileSync(resolve(dir, 'rt.js'), 'export const data = {}, Resources = [], commits = []; export const commit = (f, why) => { commits.push(why); f(); }; export function R() {} export const Refusal = Error;');
    const { viewport } = await import(resolve(dir, 'facts.js'));
    const { data, commits } = await import(resolve(dir, 'rt.js'));
    viewport({ v: [['gpuTier']], ratio: [['devicePixelRatio']], geometry: [['safeAreaTop', 'safeAreaBottom', 'segments'], { safeAreaTop: 'n', safeAreaBottom: 'n', segments: ['[]', { height: 'n', x: 'n', width: 'n', y: 'n' }] }] });
    const read = () => data.reserved.exactViewport(null, null, 'v')[0];
    expect(read()).toBe('high');
    const ratio = () => data.reserved.exactViewport(null, null, 'ratio')[0];
    expect(ratio()).toBe(2);
    globalThis.devicePixelRatio = 3;
    const before = commits.length;
    media.get('(resolution: 2dppx)')();
    expect(commits.length).toBe(before + 1);
    expect(ratio()).toBe(3);
    expect(media.has('(resolution: 2dppx)')).toBe(false);
    expect(media.has('(resolution: 3dppx)')).toBe(true);
    globalThis.devicePixelRatio = undefined;
    expect(ratio()).toBe(null);
    globalThis.exact.page.prefer({ 'gpu-tier': 'low' });
    expect(read()).toBe('low');
    expect(commits).toContain('viewport');
    document.body = { append() {} };
    document.createElement = () => ({ style: {} });
    globalThis.getComputedStyle = () => ({ paddingTop: '24px', paddingRight: '0px', paddingBottom: '34px', paddingLeft: '0px' });
    globalThis.visualViewport = { height: innerHeight };
    const geometry = () => data.reserved.exactViewport(null, null, 'geometry');
    expect(geometry()).toEqual([24, 34, [[844, 0, 390, 0]]]);
    globalThis.exact.fold.prefer({ cols: 2, rows: 1, gap: 10 });
    expect(geometry()).toEqual([24, 34, [[844, 0, 190, 0], [844, 200, 190, 0]]]);
    globalThis.exact.fold.prefer({ cols: 2, rows: 1, gap: 20 });
    expect(geometry()).toEqual([24, 34, [[844, 0, 185, 0], [844, 205, 185, 0]]]);
    expect(globalThis.exact.page.read()['gpu-tier']).toBe('low');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
