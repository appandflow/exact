import { test, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

function runtimeServer(development = false) {
  return Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/favicon.ico') { return new Response(null, { status: 204 }); }
    if (path.endsWith('/names.js')) { return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } }); }
    if (path === '/') { return new Response('<div id="exact-root"></div>', { headers: { 'content-type': 'text/html' } }); }
    const local = new URL('../..' + path, import.meta.url), file = existsSync(local) ? local : new URL('../web/' + path.split('/').pop(), import.meta.url);
    if (path.endsWith('/rt.js') && development) { return new Response(readFileSync(file, 'utf8').replace('const DEVELOPMENT_STORE = null;', 'const DEVELOPMENT_STORE = "fixture";'), { headers: { 'content-type': 'text/javascript' } }); }
    return new Response(Bun.file(file));
  } });
}

test('resource observations keep request provenance through switching, failure, rollback and recovery', async () => {
  for (const development of [true, false]) {
    const server = runtimeServer(development);
    const browser = await suiteBrowser(), page = await browser.newPage();
    try {
      await page.goto(server.url.href);
      await page.evaluate(async () => {
        const rt = await import('/host/web-js/rt.js'), owner = rt.sig('A'), completions = [];
        rt.clock.agent = true;
        rt.data.answer = () => ({ promise: new Promise(resolve => completions.push(resolve)) });
        const feed = rt.res('feed', 'feed', () => [owner()], undefined, undefined, 's', 'empty');
        window.fixture = { rt, owner, feed, completions,
          inspect: () => feed.r.observation?.(0),
          pick: v => rt.commit(() => rt.W(owner, v)),
          refresh: () => rt.commit(() => rt.R(feed)),
          finish: (i, value) => completions[i](value) };
        rt.commit(() => {});
      });
      const inspect = () => page.evaluate(() => fixture.inspect());
      const finish = async (index, value) => {
        await page.evaluate(([i, v]) => fixture.finish(i, v), [index, value]);
        await page.waitForFunction(() => fixture.rt.inflight.n === 0);
      };
      if (development) expect(await inspect()).toMatchObject({ displayed: 'fallback', pending: true, argumentsEqual: null });
      else expect(await inspect()).toBeUndefined();
      await finish(0, 'A rows');
      const first = await inspect();
      if (development) expect(first).toMatchObject({ displayed: 'current', pending: false, argumentsEqual: true });
      await page.evaluate(() => fixture.pick('B'));
      if (development) expect(await inspect()).toMatchObject({ displayed: 'retained', pending: true, argumentsEqual: false, displayedGeneration: first.displayedGeneration });
      await finish(1, 42);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'retained', pending: false, argumentsEqual: false });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(1);
      }
      await page.evaluate(() => fixture.pick('A'));
      if (development) expect(await inspect()).toMatchObject({ displayed: 'retained', pending: false, argumentsEqual: true });
      await page.evaluate(() => fixture.refresh());
      await finish(2, 'A fresh rows');
      const recovered = await inspect();
      if (development) expect(recovered.displayed).toBe('current');
      await page.evaluate(() => fixture.refresh());
      await finish(3, 42);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'retained', argumentsEqual: true, lastSuccessfulGeneration: recovered.displayedGeneration });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(2);
      }
      const before = await inspect();
      await page.evaluate(() => fixture.rt.commit(() => { fixture.rt.W(fixture.owner, 'C'); throw new fixture.rt.Refusal('refused'); }));
      expect(await inspect()).toEqual(before);
      expect(await page.evaluate(() => fixture.feed())).toBe('A fresh rows');
      await page.evaluate(() => { fixture.pick('B'); fixture.pick('C'); fixture.finish(4, 42); fixture.finish(5, 'C rows'); });
      await page.waitForFunction(() => fixture.rt.inflight.n === 0);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'current', argumentsEqual: true });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(2);
      }
      expect(await page.evaluate(() => fixture.feed())).toBe('C rows');
    } finally { await browser.close(); server.stop(true); }
  }
}, 30000);

test.each([['not pending', false], ['pending', true]])('resource pending state rolls back with its request (initially %s)', async (_, initiallyPending) => {
  const server = runtimeServer(), browser = await suiteBrowser(), page = await browser.newPage();
  try {
    await page.goto(server.url.href);
    const before = await page.evaluate(async initiallyPending => {
      globalThis.__exactRender = true;
      const rt = await import('/host/web-js/rt.js'), answer = Promise.withResolvers();
      let trigger, later, bad, controller;
      rt.data.answer = (name, [value]) => {
        if (name === 'trigger') { return { promise: answer.promise }; }
        if (name === 'later' && (value === 0) === initiallyPending) {
          return { stream: (_deliver, owner) => { controller = owner; return new Promise(() => {}); } };
        }
        if (name === 'bad' && value === 1) { throw Object.assign(new Error('source refused'), { refuse: true }); }
        return { v: 0 };
      };
      rt.mount(root => {
        trigger = rt.res('trigger', 'trigger', () => [], undefined, undefined, 'n', 0);
        later = rt.res('later', 'later', () => [trigger()], undefined, undefined, 'n', 0);
        bad = rt.res('bad', 'bad', () => [trigger()], undefined, undefined, 'n', 0);
        rt.hd(root, { headTitle: () => [trigger(), later(), bad()].join(' ') });
      });
      // A heartbeat ends the transport wait while the resource still needs a value.
      controller?.replied();
      const state = () => ({ pending: later.p(), ticket: !!later.r.ticket, title: document.title });
      window.fixture = { trigger, answer, state };
      return state();
    }, initiallyPending);
    expect(before).toEqual({ pending: initiallyPending, ticket: initiallyPending, title: '0 0 0' });
    await page.evaluate(() => fixture.answer.resolve(1));
    await page.waitForFunction(() => fixture.trigger.f());
    expect(await page.evaluate(() => fixture.state())).toEqual(before);
  } finally { await browser.close(); server.stop(true); }
}, 30000);

test.each([
  ['derive', false], ['derive', true], ['source', false], ['source', true],
])('a list element first asked in a refused commit is asked again (refusal: %s, existing pending: %s)', async (refuseAt, initiallyPending) => {
  const server = runtimeServer(), browser = await suiteBrowser(), page = await browser.newPage();
  try {
    await page.goto(server.url.href);
    await page.evaluate(async refuseAt => {
      const rt = await import('/host/web-js/rt.js'), { resEach } = await import('/host/web-js/each.js');
      rt.clock.agent = true;
      const ids = rt.sig([1]), bad = rt.sig(false), asked = [], answers = [];
      rt.data.answer = (_source, [n]) => {
        asked.push(n);
        if (n === 2 && bad() && refuseAt === 'source') { throw new rt.Refusal('source refused'); }
        const answer = Promise.withResolvers();
        answers.push(() => answer.resolve(n * 10));
        return { promise: answer.promise };
      };
      const items = resEach('items', 'item', ids, n => n, n => [n], '[n', 'n', 0);
      const share = rt.memo(() => 100 / (items().length + 1 - (bad() ? items().length + 1 : 0)), 'n', 'share');
      rt.commit(() => {});
      rt.effect(() => { document.getElementById('exact-root').textContent = `${items().join(',')} ${items.p() ? 'pending' : 'settled'} ${share()}`; });
      window.fixture = { rt, ids, bad, asked, answers,
        state: () => ({ shown: items(), pending: items.p(), ids: ids(), bad: bad(), share: share(), asked: [...asked] }),
        add: refuse => rt.commit(() => { rt.W(ids, [1, 2]); rt.W(bad, refuse); }) };
    }, refuseAt);
    if (!initiallyPending) {
      await page.evaluate(() => fixture.answers[0]());
      await page.waitForFunction(() => fixture.rt.inflight.n === 0);
    }
    const before = await page.evaluate(() => fixture.state());
    expect(before).toEqual({ shown: [initiallyPending ? 0 : 10], pending: initiallyPending, ids: [1], bad: false, share: 50, asked: [1] });
    expect(await page.evaluate(() => fixture.add(true))).toBe(false);
    expect(await page.evaluate(() => fixture.state())).toEqual({ ...before, asked: [1, 2] });
    expect(await page.evaluate(() => fixture.rt.Resources.length)).toBe(1);
    expect(await page.evaluate(() => fixture.add(false))).toBe(true);
    await page.evaluate(() => fixture.answers.forEach(finish => finish()));
    await page.waitForFunction(() => fixture.rt.inflight.n === 0);
    expect(await page.evaluate(() => fixture.state())).toEqual({
      shown: [10, 20], pending: false, ids: [1, 2], bad: false, share: 100 / 3, asked: [1, 2, 2],
    });
    expect(await page.locator('#exact-root').textContent()).toBe(`10,20 settled ${100 / 3}`);
  } finally { await browser.close(); server.stop(true); }
}, 30000);
