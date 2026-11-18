import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test('settle follows a GPU surface camera deadline and reports continuing work', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/gpu-clock-test-'));
  let session, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'GPU Clock Test', app: { id: 'test.gpu-clock', name: 'GPU Clock Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "Camera"\n');
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'gpu-clock-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      const file = path.startsWith('/shared/') ? resolve(root, 'host/web', path.slice(8))
        : resolve(dist, path === '/' ? 'index.html' : path.slice(1));
      return existsSync(file) ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    session = await open({ host: 'web', url: server.url.href });
    await session.carrier.evaluate(`(() => {
      const exact = globalThis.exact;
      exact.gpu = {
        settled: async () => [],
        clock: () => ({ pending: exact.clock.now < 250, settleAt: 250,
          reply: { world: [{ canvas: 1, quiescent: exact.clock.now >= 250 }] } }),
      };
    })()`);
    const settled = await session.clock('settle');
    expect(settled).toMatchObject({ clock: 250, settled: true, world: [{ canvas: 1, quiescent: true }] });
    await session.carrier.evaluate(`(() => {
      globalThis.exact.gpu.clock = () => ({ pending: true,
        reply: { world: [{ canvas: 1, quiescent: false, changing: ['camera'] }] } });
    })()`);
    const pending = await session.clock('settle');
    expect(pending.settled).toBe(false);
    expect(pending.world).toEqual([{ canvas: 1, quiescent: false, changing: ['camera'] }]);
  } finally {
    await session?.close(); server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120_000);
