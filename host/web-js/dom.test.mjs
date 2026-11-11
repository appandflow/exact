import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createDocument } from './dom.js';
import { webCompiler } from './module.mjs';
import { renderer } from './render.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

// The render's DOM orders nodes as DOM §4.4 does (midi.js sorts its listeners by it).
test('the render DOM compares document positions, across trees one way each', () => {
  const doc = createDocument(), root = doc.getElementById('exact-root');
  const a = doc.createElement('div'), b = doc.createElement('div');
  root.append(a, b);
  a.textContent = 'text';
  expect([a.compareDocumentPosition(b), b.compareDocumentPosition(a)]).toEqual([4, 2]);
  expect([a.compareDocumentPosition(a.firstChild), a.firstChild.compareDocumentPosition(a)]).toEqual([16 | 4, 8 | 2]);
  const loose = doc.createElement('p'), other = createDocument().getElementById('exact-root');
  for (const [x, y] of [[a, loose], [loose, other], [a, other]]) {
    const there = x.compareDocumentPosition(y), back = y.compareDocumentPosition(x);
    expect([there & 1, there & 32, back & 1, back & 32]).toEqual([1, 32, 1, 32]);
    expect([there & 6, back & 6].sort()).toEqual([2, 4]);
    expect(x.compareDocumentPosition(y)).toBe(there);
  }
});

test('the render DOM reflects link relations and image alternatives in both directions', () => {
  const doc = createDocument();
  for (const [tag, prop, value] of [['a', 'rel', 'external noopener'], ['img', 'alt', 'A person']]) {
    const el = doc.createElement(tag);
    expect(el[prop]).toBe('');
    el[prop] = value;
    expect(el.getAttribute(prop)).toBe(value);
    el.setAttribute(prop, 'authored');
    expect(el[prop]).toBe('authored');
    el.removeAttribute(prop);
    expect(el[prop]).toBe('');
    el[prop] = '';
    expect(el.getAttribute(prop)).toBe('');
  }
});

test('Bun-rendered bound links and symbols keep their attributes through adoption', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/dom-render-test-'));
  let server, browser;
  try {
    const dist = resolve(dir, 'dist');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'DOM Render Test', app: { id: 'test.dom-render', name: 'DOM Render Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component App
  state url = "https://example.com/a"
  state role = "person"
  action local
    url = "/about"
  action external
    url = "https://example.com/a"
  view
    column
      link href=url testId="lnk"
        text "site"
      link href=url target="_self" testId="authored"
        text "same tab"
      image \`symbol:\${role}\` testId="sym"
      image \`symbol:\${role}\` alt="A person" testId="named"
      button press=local testId="local"
        text "make it local"
      button press=external testId="external"
        text "make it external"
`);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', dir, '--out', dist, '--render', 'js'], { cwd: root, env: process.env, stdout: 'pipe', stderr: 'pipe' });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const render = renderer(dist);
    render.ahead = false;
    writeFileSync(resolve(dist, 'index.html'), (await render('/')).html);
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    expect(await page.evaluate(() => !!globalThis.exact?.ready)).toBe(false);
    await page.evaluate(() => { globalThis.renderedLink = document.querySelector('[data-testid="lnk"]'); });
    const read = async () => ({
      attrs: await page.evaluate(() => {
        const el = id => document.querySelector(`[data-testid="${id}"]`);
        return {
          link: ['href', 'target', 'rel'].map(k => el('lnk').getAttribute(k)),
          authored: ['target', 'rel'].map(k => el('authored').getAttribute(k)),
          symbols: ['sym', 'named'].map(id => el(id).getAttribute('alt')),
        };
      }),
      images: await page.getByRole('img').count(),
    });
    const before = await read();
    await page.getByTestId('external').click();
    await page.waitForFunction(() => globalThis.exact?.journal?.some(line => line.endsWith('adopted the document')));
    expect(await page.evaluate(() => renderedLink === document.querySelector('[data-testid="lnk"]'))).toBe(true);
    const adopted = await read();
    await page.getByTestId('local').click();
    await page.waitForFunction(() => document.querySelector('[data-testid="lnk"]').getAttribute('href') === '/about');
    const local = await read();
    await page.getByTestId('external').click();
    await page.waitForFunction(() => document.querySelector('[data-testid="lnk"]').getAttribute('href') === 'https://example.com/a');
    const external = await read();
    const expected = link => ({ attrs: { link, authored: ['_self', null], symbols: ['', 'A person'] }, images: 1 });
    const out = expected(['https://example.com/a', '_blank', 'external noopener']);
    expect({ before, adopted, local, external }).toEqual({ before: out, adopted: out, local: expected(['/about', null, null]), external: out });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 60000);
