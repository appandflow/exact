import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { pageServer } from './render.mjs';

// A render has no launch locale: it reads the base table, whose name and direction are the page's `lang` and `dir`
// (LLP 1060 D4), as the native render host writes them (host/render/src/page.rs). The manifest's `lang` is the page's
// only for a plan with no table (LLP 1115.000 D4.6).
const SOURCE = 'routes nav\n  home "/" render=build\n\ncomponent App\n  view\n    column\n      text t("hello") testId="hello"\n';
const AR = { 'strings/ar.json': '{"hello":"مرحبا"}' }, EN = { 'strings/en.json': '{"hello":"Hello"}' };
const CASES = [
  ['an Arabic base alone', { strings: { base: 'ar' } }, AR, 'ar', 'rtl', 'مرحبا'],
  ['an Arabic base with English', { strings: { base: 'ar' } }, { ...AR, ...EN }, 'ar', 'rtl', 'مرحبا'],
  ['an English base under an Arabic manifest', { lang: 'ar' }, { ...AR, ...EN }, 'en', 'ltr', 'Hello'],
  ['no table under an Arabic manifest', { lang: 'ar' }, {}, 'ar', 'ltr', 'plain'],
];
const opening = html => /<html[^>]*>/.exec(html)?.[0];
const said = html => /data-testid="hello"[^>]*>([^<]*)</.exec(html)?.[1];

for (const [name, manifest, files, lang, dir, text] of CASES) test(`a JavaScript render of ${name} is lang="${lang}" dir="${dir}"`, async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), app = mkdtempSync(resolve(root, 'target/render-language-test-'));
  try {
    writeFileSync(resolve(app, 'app.json'), JSON.stringify({ name: 'Render Language Test', app: { id: 'test.language', name: 'Render Language Test' }, host: { web: {} }, ...manifest }));
    writeFileSync(resolve(app, 'app.contract'), Object.keys(files).length ? SOURCE : SOURCE.replace('t("hello")', '"plain"'));
    if (Object.keys(files).length) mkdirSync(resolve(app, 'strings'));
    for (const [path, body] of Object.entries(files)) writeFileSync(resolve(app, path), body);
    const dist = resolve(app, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'render-language-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: app }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const built = readFileSync(resolve(dist, 'index.html'), 'utf8');
    expect([opening(built), said(built)]).toEqual([`<html lang="${lang}" dir="${dir}">`, text]);
    // `render.mjs --serve` composes its heads alike: a navigation's head, flushed before the render, and a whole page.
    const page = pageServer(dist, undefined, () => {});
    const streamed = await page('/', h => ({ 'sec-fetch-dest': 'document' })[h]);
    expect(streamed.body).toBeInstanceOf(ReadableStream);
    const flushed = await new Response(streamed.body).text();
    expect([opening(flushed), said(flushed)]).toEqual([`<html lang="${lang}" dir="${dir}">`, text]);
    const whole = await page('/?whole', () => undefined);
    expect([opening(whole.body), said(whole.body)]).toEqual([`<html lang="${lang}" dir="${dir}">`, text]);
  } finally { rmSync(app, { recursive: true, force: true }); }
}, 60000);
