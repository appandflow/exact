import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// A URL's `pathname` keeps percent-encoding, so a checkout under a folder with a space (`My Projects/exact`) became
// `My%20Projects/exact` and every script that found its root that way failed (LLP 1156.000 D7). `fileURLToPath` decodes.
test('no script finds a path from its own URL by pathname', () => {
  const root = fileURLToPath(new URL('..', import.meta.url));
  const found = spawnSync('git', ['grep', '-n', 'import\\.meta\\.url)\\.pathname', '--', '*.mjs', '*.js', '*.ts'], { cwd: root, encoding: 'utf8' });
  expect(found.stdout).toBe('');
  expect(fileURLToPath(new URL('file:///tmp/My%20Projects/exact/'))).toBe('/tmp/My Projects/exact/');
});
