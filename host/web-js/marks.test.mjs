import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Time-to-interactive's outstanding work on the web (marks.js `outstanding`), with marks.js and held.js as they are,
// over a stand-in rt.js. The test counts tickets as `send` does: a stream leaves `inflight` at its first message and
// keeps its ticket (host/web/tests/js-runtime.test.mjs, "a stream answer settles per message").
const ROOT = resolve(import.meta.dirname, '../..');

test('an open stream hides none of the page loading that holds time-to-interactive, and a let-go request adds none', async () => {
  const directory = mkdtempSync(resolve(tmpdir(), 'exact-marks-'));
  cpSync(resolve(ROOT, 'host/web-js/marks.js'), resolve(directory, 'marks.js'));
  cpSync(resolve(ROOT, 'host/web-js/held.js'), resolve(directory, 'held.js'));
  writeFileSync(resolve(directory, 'rt.js'), `export const inflight = { n: 0 }, Resources = [], Mutations = [], clock = { timers: [] }, After = [], Hosts = {}, Routes = [];
export const routeAt = () => 0, busy = () => [];\n`);
  try {
    const rt = await import(pathToFileURL(resolve(directory, 'rt.js')).href);
    const { counted } = await import(pathToFileURL(resolve(directory, 'held.js')).href);
    const { outstanding } = await import(pathToFileURL(resolve(directory, 'marks.js')).href);
    const setWaiting = (ticket, waiting) => { ticket.waiting = waiting; counted(ticket, rt.inflight); };
    const stream = {}, save = {}, feed = { name: 'feed', placeholder: true, ticket: stream };
    rt.Resources.push(feed);
    rt.Mutations.push({ ticket: save });
    setWaiting(stream, true);
    expect(outstanding()).toEqual(['requests:feed']);
    // Deliver the stream's first message: its resource shows it and keeps the ticket for the messages to come
    setWaiting(stream, false);
    feed.placeholder = false;
    // Send a mutation's request: `inflight` counts it, and it holds no TTI
    setWaiting(save, true);
    expect(outstanding()).toEqual([]);
    // Start the page's own loading (a Markdown module, say)
    rt.inflight.n++;
    expect(outstanding()).toEqual(['loading:1']);
    // Add a request that its resource let go: it stays in `inflight` until its reply, which lands nowhere, and holds no TTI
    setWaiting({}, true);
    expect(outstanding()).toEqual(['loading:1']);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
