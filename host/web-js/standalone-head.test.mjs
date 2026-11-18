import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

function site(app, source) {
  mkdirSync(app, { recursive: true });
  writeFileSync(resolve(app, 'app.contract'), source);
  writeFileSync(resolve(app, 'app.ts'), "import type { Answer, Result, Sources } from './app.contract.d.ts';\nexport const appId = 'test.site';\nexport const grants = '';\n"
    + "const sources: Sources = { note: ([what]): Result<'note'> => ({ text: `The note, ${what}.` }) };\n"
    + 'export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);\n');
}

test('a standalone app under viewport-fit=cover draws under the Home Screen status bar in its shell and a page built ahead', () => {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..'), dir = mkdtempSync(resolve(tmpdir(), 'exact-site-standalone-')), app = resolve(dir, 'site'), out = resolve(app, 'dist');
  const tags = '<meta name="mobile-web-app-capable" content="yes">\n<meta name="apple-mobile-web-app-capable" content="yes">\n', bar = '<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">\n';
  try {
    for (const fit of ['cover', 'auto']) {
      site(app, `shape Note\n  text: string\n\nroutes nav\n  home "/" render=build\n\ncomponent App\n  resource note = note("rendered") as shape Note\n  view\n    column viewport-fit="${fit}"\n      text note.text\n`);
      writeFileSync(resolve(app, 'app.json'), JSON.stringify({ name: 'Site', display: 'standalone', app: { id: 'test.site', name: 'Site' }, host: { web: {} } }));
      const built = spawnSync(process.execPath, ['host/web/build.mjs', app, '--out', out], { cwd: root, env: { ...process.env, EXACT_APP_DIR: app }, encoding: 'utf8' });
      expect(built.status, built.stdout + built.stderr).toBe(0);
      for (const name of ['shell.html', 'index.html']) expect([fit, name, ...[tags, bar].map(t => readFileSync(resolve(out, name), 'utf8').split(t).length - 1)]).toEqual([fit, name, 1, fit === 'cover' ? 1 : 0]);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 300_000);
