#!/usr/bin/env bun
// Clear out Cargo output no build has used for a week. Every source, feature, or path change
// compiles a unit under a new hash and Cargo never removes the old one, so a checkout's target/
// only grows (one reached 116 GB, most of it stale copies of each app's debug staticlib).
// `resolveApp` starts this, detached, at most once an hour; `bun scripts/sweep.mjs <target>` runs it now.
//
// "Used" is measured, not guessed. Cargo reads the fingerprint of every unit a build considers,
// even when it compiles nothing, and never reads one it does not build. Each sweep sets those
// files' access times back to RESET; the kernel updates an access time older than the file's
// mtime on the next read (macOS, and Linux relatime), so one read since the last sweep shows.
// The sweep never writes a modification time: Cargo compares a unit's inputs against its
// dep-info file's mtime to the nanosecond, so even rounding it to the millisecond can make an
// input written in the same millisecond read as newer, and the unit rebuild.
// A unit is `.fingerprint/<package>-<hash>` plus every `deps/` file and `build/` directory with
// that hash. Incremental caches carry rustc's own names, so they go by when they last compiled.
//
// Cargo holds `<profile>/.cargo-lock` (flock) while it builds into a profile directory. The sweep
// takes the same lock without waiting and skips a directory a build holds; under the lock it only
// renames, fingerprints first, so a unit is never half there. It deletes after letting go.
import { closeSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { basename, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const KEEP = 7 * 86_400_000, EVERY = 3_600_000;
const RESET = 86_400; // 1970-01-02, in seconds: older than any real mtime
const LEDGER = '.exact-sweep.json', LOG = '.exact-sweep.log', ASIDE = '.exact-swept';
const HASH = /-([0-9a-f]{16})(?:\.|$)/;

/** Start a sweep of `target` in the background if the last one finished over an hour ago. A
 * sweep that failed leaves its error in the log, reported on every build until one succeeds. */
export function startSweep(target, now = Date.now()) {
  // This is optional cache housekeeping. Windows has neither this flock
  // protocol nor reliable immediate access-time updates; keep its build units.
  if (process.platform === 'win32') return;
  if (!existsSync(target)) return;
  const log = resolve(target, LOG);
  if (existsSync(log) && statSync(log).size) process.stderr.write(`target sweep failed (${log}):\n${readFileSync(log, 'utf8')}`);
  const ledger = resolve(target, LEDGER);
  if (existsSync(ledger) && now - JSON.parse(readFileSync(ledger, 'utf8')).at < EVERY) return;
  const err = openSync(log, 'a');
  const child = spawn(process.execPath, [fileURLToPath(import.meta.url), resolve(target)], { cwd: import.meta.dirname, detached: true, stdio: ['ignore', 'ignore', err] });
  child.unref();
  closeSync(err);
  return child;
}

async function libc() {
  const { dlopen, FFIType } = await import('bun:ffi');
  const mac = process.platform === 'darwin';
  const { symbols } = dlopen(mac ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', {
    flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 },
    utimensat: { args: [FFIType.i32, FFIType.ptr, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  });
  // AT_FDCWD and UTIME_OMIT, from <fcntl.h> and <sys/stat.h> on each system (POSIX utimensat).
  const [AT_FDCWD, UTIME_OMIT] = mac ? [-2, -2] : [-100, (1 << 30) - 2];
  // Two 64-bit timespecs: the access time is RESET, the modification time is left as it is.
  const times = new BigInt64Array([BigInt(RESET), 0n, 0n, BigInt(UTIME_OMIT)]);
  return {
    // LOCK_EX | LOCK_NB, the same flock Cargo takes. Closing the descriptor releases it.
    lock: path => { const fd = openSync(path, 'a'); if (symbols.flock(fd, 6) === 0) return fd; closeSync(fd); return null; },
    resetAccess: path => {
      if (symbols.utimensat(AT_FDCWD, Buffer.from(`${path}\0`), times, 0) !== 0) throw new Error(`${path}: could not reset its access time`);
    },
  };
}

/** Without access times the sweep cannot tell a used unit from an abandoned one, so it refuses. */
function assertAccessTimes(target, resetAccess) {
  const probe = resolve(target, `${LEDGER}.probe`);
  writeFileSync(probe, 'x');
  resetAccess(probe);
  readFileSync(probe);
  const seen = statSync(probe).atimeMs > RESET * 1000;
  rmSync(probe);
  if (!seen) throw new Error(`${target}: this filesystem does not record reads (noatime), so the sweep cannot tell which units builds use`);
}

const profiles = target => [target, ...readdirSync(target, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => resolve(target, e.name))]
  .flatMap(dir => readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => resolve(dir, e.name)))
  .filter(dir => existsSync(resolve(dir, '.fingerprint')));

/** Sweep `target` as of `now`. Returns what was moved out, per profile directory. */
export async function sweep(target, now = Date.now()) {
  if (process.platform === 'win32') throw new Error('Cargo cache sweeping is unavailable on Windows: its lock and access-time semantics have not been verified');
  const { lock, resetAccess } = await libc(), own = lock(resolve(target, `${LEDGER}.lock`));
  if (own === null) return {};
  try {
    assertAccessTimes(target, resetAccess);
    const path = resolve(target, LEDGER), old = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')).units : {};
    const units = {}, removed = {}, aside = resolve(target, ASIDE, String(now));
    let n = 0;
    const move = p => { mkdirSync(aside, { recursive: true }); renameSync(p, resolve(aside, String(n++))); };
    for (const dir of profiles(target)) {
      const at = relative(target, dir), fd = lock(resolve(dir, '.cargo-lock'));
      if (fd === null) { // a build holds it: keep what we knew, look again next time
        for (const [key, t] of Object.entries(old)) if (key.startsWith(at + '#')) units[key] = t;
        continue;
      }
      try {
        const stale = new Set(), fingerprints = resolve(dir, '.fingerprint');
        for (const name of readdirSync(fingerprints)) {
          const hash = HASH.exec(name)?.[1];
          if (!hash) continue;
          const unit = resolve(fingerprints, name), files = readdirSync(unit).map(f => resolve(unit, f));
          const key = `${at}#${hash}`, read = files.some(f => statSync(f).atimeMs > RESET * 1000);
          units[key] = read || !(key in old) ? now : old[key];
          if (now - units[key] > KEEP) { stale.add(hash); delete units[key]; move(unit); continue; }
          for (const f of files) resetAccess(f);
        }
        let count = stale.size;
        for (const sub of ['deps', 'build', 'examples']) {
          const d = resolve(dir, sub);
          if (existsSync(d)) for (const name of readdirSync(d)) if (stale.has(HASH.exec(name)?.[1])) move(resolve(d, name));
        }
        const incremental = resolve(dir, 'incremental');
        if (existsSync(incremental)) for (const name of readdirSync(incremental)) {
          const cache = resolve(incremental, name);
          const last = Math.max(statSync(cache).mtimeMs, ...readdirSync(cache).map(s => statSync(resolve(cache, s)).mtimeMs));
          if (now - last > KEEP) { move(cache); count++; }
        }
        if (count) removed[at] = count;
      } finally { closeSync(fd); }
    }
    writeFileSync(`${path}.tmp`, JSON.stringify({ at: now, units }));
    renameSync(`${path}.tmp`, path);
    rmSync(resolve(target, ASIDE), { recursive: true, force: true });
    writeFileSync(resolve(target, LOG), '');
    return removed;
  } finally { closeSync(own); }
}

// `clean` empties an idle app's target/ of what its builds remake, for a tool that reclaims disk space. Exact keeps
// records there too, so it keeps: `traces/` (saved traces), `web-receipts/` and `native-receipts/` (the builds runs
// were recorded against, scripts/evidence.mjs), and each production build in `bake/` and `clients/`, whose receipt
// `push` reads to update installed binaries and whose dSYM symbolicates them. A name it does not list is kept too,
// so an author's file there, or an output added later, stays until it is listed here.
const REMADE = new Set(['.apple-cargo-locks', '.claims', '.rustc_info.json', 'CACHEDIR.TAG', 'apple-extension-module-cache',
  'apple-modules', 'apple-painted', 'apple-swift', 'bake-compiled', 'bake-input-hashes.json', 'bake-inputs', 'bake-metadata',
  'dev-gpu', 'exact-filesystem-tool', 'metal-kept', 'module-dev', 'native-reuse', 'native-runs', 'proof-web',
  'proof-web-inputs.json', 'push-tools', 'rust-bake', 'rust-dev', 'rust-modules', 'rust-sign', 'tmp', 'web-bakes', 'web-js-gpu',
  'web-js-modules', 'web-loops', 'web-opt', 'web-runs']);
const REMADE_PREFIX = /^(web-dist|web-js-dev-|web-check-|push-ts-|push-bake-|push-graph-|proof-web-copy-|kept-crates-|inspect-)/;
const RECORDS = new Set(['traces', 'web-receipts', 'native-receipts']);
// The trust directory's depth: bake/<source>/<id>/<trust> (bakeOutput, scripts/app.mjs) and
// clients/<source>/<id>/<destination>/<target>/<composition>/<trust> (appleArtifacts, host/apple/build.mjs). The Mac
// bundle assembled beside them, clients/<source>/<id>/macos/<name>.app, is kept: its trust is not in its path.
const TRUSTED = { bake: 3, clients: 6 };
// The flock files builds hold: Cargo's, build-lock.mjs's, claimBuildOutput's `.flock`, SwiftPM's scratch `.lock`.
const LOCK = /^\.cargo-lock$|\.(lock|flock)$/;
const NO_LOCKS = new Set(['.fingerprint', 'build', 'deps', 'examples', 'incremental', 'node_modules']);

const isDirectory = path => { try { return statSync(path).isDirectory(); } catch { return false; } };
const cargoOutput = path => existsSync(resolve(path, '.fingerprint'))
  || isDirectory(path) && readdirSync(path).some(name => existsSync(resolve(path, name, '.fingerprint')));
const atDepth = (dir, depth) => depth === 0 || dir.endsWith('.app') ? [dir]
  : readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).flatMap(e => atDepth(resolve(dir, e.name), depth - 1));
function locksIn(path, depth, out) {
  if (!isDirectory(path)) { if (LOCK.test(basename(path))) out.add(path); return; }
  for (const e of readdirSync(path, { withFileTypes: true })) {
    const at = resolve(path, e.name);
    if (e.isFile() && LOCK.test(e.name)) out.add(at);
    else if (e.isDirectory() && depth > 0 && !NO_LOCKS.has(e.name)) locksIn(at, depth - 1, out);
  }
}

/** What `clean` would remove from `target` and what it keeps, each an absolute path. */
function cleanable(target) {
  const remove = [], kept = [];
  for (const name of readdirSync(target)) {
    const path = resolve(target, name);
    if (name.startsWith(LEDGER) || name === LOG || name === ASIDE) continue;
    if (name in TRUSTED && isDirectory(path)) {
      for (const build of atDepth(path, TRUSTED[name])) {
        if (build.endsWith('/development')) remove.push(build);
        else kept.push({ path: build, why: build.endsWith('/production') ? 'record' : 'unlisted' });
      }
    } else if (RECORDS.has(name)) kept.push({ path, why: 'record' });
    else if (REMADE.has(name) || REMADE_PREFIX.test(name) || cargoOutput(path)) remove.push(path);
    else kept.push({ path, why: 'unlisted' });
  }
  return { remove, kept };
}

/** Remove what `cleanable` lists, all or nothing: while a build holds a lock in or above any of it, or a drive leases a
 * web build in it, nothing is removed and `held` names those. It takes the sweep's lock first, waiting up to `wait` ms,
 * so a sweep `resolveApp` just started finishes first. A build that starts while it runs may still lose its output:
 * a caller cleans an app it knows is idle. */
export async function clean(target, { dryRun = false, now = Date.now(), wait = 60_000 } = {}) {
  if (process.platform === 'win32') throw new Error('clean is unavailable on Windows: its lock semantics have not been verified');
  const { lock } = await libc(), sweepLock = resolve(target, `${LEDGER}.lock`), held = [], fds = [];
  let own = lock(sweepLock);
  for (const until = Date.now() + wait; own === null && Date.now() < until;) {
    await new Promise(ok => setTimeout(ok, 100));
    own = lock(sweepLock);
  }
  if (own === null) return { status: 'busy', remove: [], kept: [], held: [sweepLock] };
  try {
    const { remove, kept } = cleanable(target), locks = new Set();
    for (const path of remove) {
      locksIn(path, 6, locks);
      for (let dir = resolve(path, '..'); dir.startsWith(target); dir = resolve(dir, '..')) locksIn(dir, 0, locks);
    }
    locks.delete(sweepLock);
    for (const path of locks) {
      let fd = null;
      try { fd = lock(path); } catch (error) { if (error.code === 'ENOENT') continue; throw error; }
      if (fd === null) held.push(path); else fds.push(fd);
    }
    const leased = remove.filter(path => path.endsWith('.leases'));
    if (leased.length) {
      const { leasedGenerations } = await import('../host/web/generations.mjs');
      for (const path of leased) for (const id of leasedGenerations(path.slice(0, -'.leases'.length))) held.push(resolve(path, id));
    }
    if (held.length) return { status: 'busy', remove: [], kept, held };
    if (!dryRun) {
      // The sweep removes this directory at the end of every run, so a clean that stops part way is finished then.
      const aside = resolve(target, ASIDE, `clean-${now}`);
      mkdirSync(aside, { recursive: true });
      remove.forEach((path, n) => renameSync(path, resolve(aside, String(n))));
      for (const fd of fds.splice(0)) closeSync(fd);
      rmSync(resolve(target, ASIDE), { recursive: true, force: true });
    }
    return { status: 'ok', remove, kept, held };
  } finally {
    for (const fd of fds) closeSync(fd);
    closeSync(own);
  }
}

/** `clean <app-dir> [--dry-run] [--json]`: the exit code. With `--json`, one object on stdout however it ends:
 * `{version: 1, status: "ok"|"busy"|"refused"|"failed", dryRun, target, remove, kept, held, error?}`. */
async function cleanCommand(args) {
  const json = args.includes('--json'), dryRun = args.includes('--dry-run'), words = args.filter(a => !['--json', '--dry-run'].includes(a));
  let target = null;
  const report = (status, { remove = [], kept = [], held = [], error } = {}) => {
    if (json) console.log(JSON.stringify({ version: 1, status, dryRun, target, remove, kept, held, ...(error ? { error } : {}) }));
    else {
      for (const path of remove) console.log(`${dryRun ? 'would remove' : 'removed'} ${path}`);
      for (const { path, why } of kept) console.log(`kept ${path} (${why})`);
      if (held.length) console.error(`clean: in use, so nothing was removed: ${held.join(', ')}`);
      if (error) console.error(`clean: ${error}`);
    }
    return { ok: 0, busy: 1, refused: 2, failed: 1 }[status];
  };
  if (words.length !== 1 || words[0].startsWith('-')) return report('refused', { error: 'usage: bun scripts/sweep.mjs clean <app-dir> [--dry-run] [--json]' });
  if (process.env.CARGO_TARGET_DIR) return report('refused', { error: 'CARGO_TARGET_DIR is set: that target is shared, so clean leaves it alone' });
  let app;
  try { app = (await import('./app.mjs')).resolveApp(resolve(words[0])); }
  catch (error) { return report('refused', { error: error.message }); }
  target = app.target;
  if (target !== resolve(app.dir, 'target')) return report('refused', { error: `${target} is shared with the app's workspace or this checkout, so clean leaves it alone` });
  if (!existsSync(target)) return report('ok');
  try { const result = await clean(target, { dryRun }); return report(result.status, result); }
  catch (error) { return report(process.platform === 'win32' ? 'refused' : 'failed', { error: error.message }); }
}

if (import.meta.main) {
  if (process.argv[2] === 'clean') process.exit(await cleanCommand(process.argv.slice(3)));
  const removed = await sweep(resolve(process.argv[2] ?? 'target'));
  if (process.stdout.isTTY) console.log(Object.keys(removed).length ? removed : 'nothing unused for a week');
}
