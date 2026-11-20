import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { relative, resolve } from 'node:path';
import process from 'node:process';
import { clean, startSweep, sweep } from './sweep.mjs';

const DAY = 86_400_000;
const A = '0123456789abcdef', B = 'fedcba9876543210';

function target() {
  const t = mkdtempSync(resolve(tmpdir(), 'exact-sweep-')), d = resolve(t, 'debug');
  for (const [pkg, hash] of [['a', A], ['b', B]]) {
    mkdirSync(resolve(d, '.fingerprint', `${pkg}-${hash}`), { recursive: true });
    writeFileSync(resolve(d, '.fingerprint', `${pkg}-${hash}`, `lib-${pkg}`), hash);
    mkdirSync(resolve(d, 'deps'), { recursive: true });
    writeFileSync(resolve(d, 'deps', `lib${pkg}-${hash}.rlib`), '');
    writeFileSync(resolve(d, 'deps', `${pkg}-${hash}.cgu.0.rcgu.o`), '');
    mkdirSync(resolve(d, 'build', `${pkg}-${hash}`, 'out'), { recursive: true });
  }
  mkdirSync(resolve(d, 'incremental', 'a-2ajo1mk2vrgcf', 's-old'), { recursive: true });
  return t;
}
const has = (t, ...p) => existsSync(resolve(t, 'debug', ...p));

test.skipIf(process.platform === 'win32')('a detached sweep ignores the caller’s TypeScript config and resolves its target there', async () => {
  const directory = mkdtempSync(resolve(tmpdir(), 'exact-sweep-caller-'));
  const cache = resolve(directory, 'target');
  mkdirSync(cache);
  writeFileSync(resolve(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: { moduleSuffixes: ['.ios', ''] } }));
  const previous = process.cwd();
  let child;
  try {
    process.chdir(directory);
    child = startSweep('./target');
  } finally { process.chdir(previous); }
  try {
    const status = await new Promise((ok, fail) => {
      child.once('error', fail);
      child.once('close', ok);
    });
    assert.equal(status, 0, readFileSync(resolve(cache, '.exact-sweep.log'), 'utf8'));
    assert.ok(existsSync(resolve(cache, '.exact-sweep.json')));
    assert.equal(readFileSync(resolve(cache, '.exact-sweep.log'), 'utf8'), '');
  } finally { rmSync(directory, { recursive: true, force: true }); }
}, 30_000);

test.skipIf(process.platform === 'win32')('a unit no build has read for a week goes; one read since the last sweep stays', async () => {
  const t = target(), now = Date.now(), b = resolve(t, 'debug/.fingerprint', `b-${B}`, 'lib-b');
  try {
    const written = statSync(b, { bigint: true }).mtimeNs;
    assert.deepEqual(await sweep(t, now), {}); // first sight: everything is new
    // Only the access time is reset: Cargo compares inputs against these mtimes to the nanosecond.
    assert.equal(statSync(b, { bigint: true }).mtimeNs, written);
    assert.equal(statSync(b).atimeMs, 86_400_000);
    readFileSync(b); // a build considers b
    const old = new Date(now - 8 * DAY);
    utimesSync(resolve(t, 'debug/incremental/a-2ajo1mk2vrgcf/s-old'), old, old);
    utimesSync(resolve(t, 'debug/incremental/a-2ajo1mk2vrgcf'), old, old);
    assert.deepEqual(await sweep(t, now + 8 * DAY), { debug: 2 });
    assert.ok(!has(t, '.fingerprint', `a-${A}`) && !has(t, 'deps', `liba-${A}.rlib`) && !has(t, 'deps', `a-${A}.cgu.0.rcgu.o`) && !has(t, 'build', `a-${A}`));
    assert.ok(!has(t, 'incremental', 'a-2ajo1mk2vrgcf'));
    assert.ok(has(t, '.fingerprint', `b-${B}`) && has(t, 'deps', `libb-${B}.rlib`) && has(t, 'build', `b-${B}`));
    assert.ok(!existsSync(resolve(t, '.exact-swept')));
  } finally { rmSync(t, { recursive: true, force: true }); }
});

test.skipIf(process.platform === 'win32')('a profile directory a build holds is left alone, and its units keep their standing', async () => {
  const t = target(), now = Date.now();
  await sweep(t, now);
  // Hold `debug/.cargo-lock` the way Cargo does (flock), from another process.
  const holder = spawn('perl', ['-MFcntl=:flock', '-e', 'open my $f, ">>", $ARGV[0] or die; flock($f, LOCK_EX) or die; print "held\\n"; $| = 1; sleep 30', resolve(t, 'debug/.cargo-lock')], { stdio: ['ignore', 'pipe', 'inherit'] });
  try {
    await new Promise(ok => holder.stdout.once('data', ok));
    assert.deepEqual(await sweep(t, now + 8 * DAY), {});
    assert.ok(has(t, '.fingerprint', `a-${A}`));
    const units = JSON.parse(readFileSync(resolve(t, '.exact-sweep.json'), 'utf8')).units;
    assert.equal(units[`debug#${A}`], now);
    holder.kill();
    await new Promise(ok => holder.once('exit', ok));
    assert.deepEqual(await sweep(t, now + 9 * DAY), { debug: 3 }); // a and b unread since, and a week past the cache
    assert.deepEqual(readdirSync(resolve(t, 'debug/.fingerprint')), []);
  } finally { holder.kill(); rmSync(t, { recursive: true, force: true }); }
});

test.skipIf(process.platform !== 'win32')('Windows keeps Cargo units instead of using an unverified lock protocol', async () => {
  const t = target();
  try {
    startSweep(t);
    await assert.rejects(sweep(t), /unavailable on Windows/);
    assert.ok(has(t, '.fingerprint', `a-${A}`));
    assert.ok(has(t, '.fingerprint', `b-${B}`));
    assert.ok(!existsSync(resolve(t, '.exact-sweep.json')));
    assert.ok(!existsSync(resolve(t, '.exact-swept')));
  } finally { rmSync(t, { recursive: true, force: true }); }
});

// An app's target/ as its builds leave it: Cargo output, Exact's remade outputs, its records, and an author's file.
function appTarget() {
  const t = mkdtempSync(resolve(tmpdir(), 'exact-clean-'));
  const files = {
    'debug/.fingerprint/a-0123456789abcdef/lib-a': '', 'debug/.cargo-lock': '', 'aarch64-apple-ios/debug/.fingerprint/x': '',
    'apple-swift/ios-simulator-17.0/.lock': '', 'apple-swift/ios-simulator-17.0.lock.flock': '', 'web-dist/index.html': '', 'web-dist.lock': '',
    'web-js-dev-compiler-app/x': '', 'kept-crates-host-dev.json': '{}', 'CACHEDIR.TAG': '',
    'bake/k/app.id/development/ios-x.build.json': '{}', 'bake/k/app.id/production/ios-x.build.json': '{}',
    'clients/k/app.id/.apple-build.lock.flock': '', 'clients/k/app.id/ios-simulator/t/embedded/development/standalone/App': '',
    'clients/k/app.id/ios/t/embedded/production/standalone/App.dSYM/x': '', 'clients/k/app.id/macos/App.app/Contents/Info.plist': '',
    'bake-compiled/0123/lib.rs': '', '.apple-cargo-locks/t/liba.lock': '',
    'traces/trace-1.json': '{}', 'web-receipts/0123/.exact-build.json': '{}', 'native-receipts/0123/receipt.json': '{}',
    'dist/App.app/x': '', 'screenshot.png': '',
  };
  for (const [path, text] of Object.entries(files)) { mkdirSync(resolve(t, path, '..'), { recursive: true }); writeFileSync(resolve(t, path), text); }
  return t;
}
const REMOVED = ['.apple-cargo-locks', 'CACHEDIR.TAG', 'aarch64-apple-ios', 'apple-swift', 'bake-compiled', 'bake/k/app.id/development', 'clients/k/app.id/ios-simulator/t/embedded/development',
  'debug', 'kept-crates-host-dev.json', 'web-dist', 'web-dist.lock', 'web-js-dev-compiler-app'];
const KEPT = [['bake/k/app.id/production', 'record'], ['clients/k/app.id/ios/t/embedded/production', 'record'],
  ['clients/k/app.id/macos/App.app', 'unlisted'], ['dist', 'unlisted'],
  ['native-receipts', 'record'], ['screenshot.png', 'unlisted'], ['traces', 'record'], ['web-receipts', 'record']];
const relativeTo = (t, { remove, kept }) => ({ remove: remove.map(p => relative(t, p)).sort(), kept: kept.map(k => [relative(t, k.path), k.why]).sort() });

test.skipIf(process.platform === 'win32')('clean removes what builds remake, keeps records and what it does not list, and a dry run removes nothing', async () => {
  const t = appTarget();
  try {
    // A ledger over an hour old: the sweep resolveApp starts runs beside clean, which waits for it.
    writeFileSync(resolve(t, '.exact-sweep.json'), JSON.stringify({ at: 0, units: {} }));
    const child = startSweep(t), swept = new Promise(ok => child.once('close', ok));
    const dry = await clean(t, { dryRun: true });
    assert.equal(dry.status, 'ok', JSON.stringify(dry.held));
    assert.deepEqual(relativeTo(t, dry), { remove: REMOVED, kept: KEPT });
    for (const path of REMOVED) assert.ok(existsSync(resolve(t, path)), path);
    await swept;
    const done = await clean(t);
    assert.equal(done.status, 'ok');
    assert.deepEqual(relativeTo(t, done), { remove: REMOVED, kept: KEPT });
    for (const path of REMOVED) assert.ok(!existsSync(resolve(t, path)), path);
    for (const [path] of KEPT) assert.ok(existsSync(resolve(t, path)), path);
    assert.ok(!existsSync(resolve(t, '.exact-swept')));
  } finally { rmSync(t, { recursive: true, force: true }); }
}, 30_000);

test.skipIf(process.platform === 'win32')('clean removes nothing while a build holds a lock in or above what it would remove', async () => {
  for (const lockPath of ['debug/.cargo-lock', 'clients/k/app.id/.apple-build.lock.flock']) {
    const t = appTarget();
    const holder = spawn('perl', ['-MFcntl=:flock', '-e', 'open my $f, ">>", $ARGV[0] or die; flock($f, LOCK_EX) or die; print "held\\n"; $| = 1; sleep 30', resolve(t, lockPath)], { stdio: ['ignore', 'pipe', 'inherit'] });
    try {
      await new Promise(ok => holder.stdout.once('data', ok));
      const busy = await clean(t);
      assert.equal(busy.status, 'busy');
      assert.deepEqual(busy.held, [resolve(t, lockPath)]);
      assert.deepEqual(busy.remove, []);
      for (const path of REMOVED) assert.ok(existsSync(resolve(t, path)), path);
    } finally { holder.kill(); rmSync(t, { recursive: true, force: true }); }
  }
}, 30_000);

test.skipIf(process.platform === 'win32')('clean leaves a shared target alone', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-clean-shared-'));
  try {
    const run = spawnSync(process.execPath, [resolve(import.meta.dir, 'sweep.mjs'), 'clean', dir, '--json'], { encoding: 'utf8', env: { ...process.env, CARGO_TARGET_DIR: dir } });
    assert.equal(run.status, 2, run.stderr);
    assert.deepEqual({ ...JSON.parse(run.stdout), error: undefined }, { version: 1, status: 'refused', dryRun: false, target: null, remove: [], kept: [], held: [], error: undefined });
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
