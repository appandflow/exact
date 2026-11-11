// The Android carrier's reader variables, drive setup and reverse mappings (agent-android.mjs).
// Stand-in adb devices exercise relay cleanup without an emulator or a device.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pollUntil } from './agent-launch.mjs';

test('the Android reader variable filter and activity allowlist include the auth fixture; loopback URLs select ports', async () => {
  const { readerVariables, reversedPorts } = await import('./agent-android.mjs');
  const vars = readerVariables({ EXACT_AGENT: '1', EXACT_AGENT_STORAGE: 's', EXACT_AUTH_FIXTURE: 'http://127.0.0.1:4791', EXACT_APP_DIR: '/a', HOME: '/h' });
  expect(vars).toEqual({ EXACT_AGENT: '1', EXACT_AGENT_STORAGE: 's', EXACT_AUTH_FIXTURE: 'http://127.0.0.1:4791' });
  expect(reversedPorts(vars)).toEqual(['4791']);
  expect(reversedPorts({ EXACT_AUTH_FIXTURE: 'http://[::1]:4791' })).toEqual(['4791']);
  expect(reversedPorts({ EXACT_DEV_PLAN: 'http://localhost:8080/', EXACT_AUTH_FIXTURE: 'http://127.0.0.1:8080' })).toEqual(['8080']);
  expect(reversedPorts({ EXACT_DEV_PLAN: 'https://example.com/plan' })).toEqual([]);
  const activity = readFileSync(resolve(import.meta.dir, '../host/android/canvas/src/main/java/dev/exact/canvas/ExactActivity.kt'), 'utf8');
  expect(Object.keys(vars).map((name) => `x_${name.slice('EXACT_'.length).toLowerCase()}`).filter((extra) => !activity.includes(`"${extra}"`))).toEqual([]);
});

// The stand-in device records adb's arguments and supplies only the reader's ready message.
function androidDriveFixture(reader, destination = null, reacquire = false, failedRemoval = null) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-adb-')), calls = resolve(dir, 'calls'), mappings = resolve(dir, 'mappings');
  writeFileSync(mappings, JSON.stringify(destination ? { 'tcp:4791': destination } : {}));
  let disconnected; const closed = new Promise(done => { disconnected = done; });
  const server = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: {
    open(socket) { socket.write('{"ready":true}\n'); }, data() {}, close() { disconnected(); },
  } });
  mkdirSync(resolve(dir, 'platform-tools'));
  writeFileSync(resolve(dir, 'platform-tools/adb'), `#!${process.execPath}
import { appendFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
const args = process.argv.slice(2);
appendFileSync(${JSON.stringify(calls)}, JSON.stringify(args) + '\\n');
const cmd = args.slice(2);
if (cmd[0] === 'reverse') {
  const file = ${JSON.stringify(mappings)}, mappings = JSON.parse(readFileSync(file, 'utf8'));
  if (cmd[1] === '--list') console.log(Object.entries(mappings).map(([from, to]) => 'UsbFfs ' + from + ' ' + to).join('\\n'));
  if (cmd[1] === '--no-rebind') { if (mappings[cmd[2]]) process.exit(1); mappings[cmd[2]] = cmd[3]; }
  if (cmd[1] === '--remove' && cmd[2] === 'tcp:4792' && ${JSON.stringify(failedRemoval)} && !existsSync(file + '.failed')) {
    writeFileSync(file + '.failed', '');
    if (${JSON.stringify(failedRemoval)} === 'replaced') { mappings[cmd[2]] = 'tcp:9001'; writeFileSync(file, JSON.stringify(mappings)); }
    process.exit(1);
  }
  if (cmd[1] === '--remove') { if (!mappings[cmd[2]]) process.exit(1); delete mappings[cmd[2]]; if (${reacquire} && !existsSync(file + '.acquired')) { mappings[cmd[2]] = 'tcp:9000'; writeFileSync(file + '.acquired', ''); } }
  writeFileSync(file, JSON.stringify(mappings));
}
if (cmd[0] === 'forward' && cmd[1] === 'tcp:0') console.log(${server.port});
if (cmd[0] === 'shell' && cmd[1] === '-T') console.log('{"ready":true}');
`, { mode: 0o755 });
  const bin = resolve(dir, reader ? 'app.apk' : 'app'); writeFileSync(bin, 'stand-in app');
  const env = { ...process.env, ANDROID_HOME: dir, ANDROID_SERIAL: 'fixture-device',
    EXACT_AGENT: '1', EXACT_AUTH_FIXTURE: 'http://127.0.0.1:4791' };
  for (const name of ['EXACT_DEV_PLAN', 'EXACT_PLAN', 'EXACT_WORLD', 'EXACT_LAUNCH_URL']) delete env[name];
  return { port: server.port, app: { id: 'dev.exact.fixture', dir }, bin, env, closed,
    commands: () => readFileSync(calls, 'utf8').trim().split('\n').map(JSON.parse),
    mappings: () => JSON.parse(readFileSync(mappings, 'utf8')), stopSocket: () => server.stop(true),
    dispose() { server.stop(true); rmSync(dir, { recursive: true, force: true }); },
  };
}
for (const reader of [false, true]) for (const present of [false, true]) test(`the Android ${reader ? 'reader' : 'headless'} drive ${present ? 'keeps an existing reverse' : 'adds and removes its reverse'} and passes the fixture`, async () => {
    const { androidDeploy } = await import('./agent-android.mjs');
    const fixture = androidDriveFixture(reader, present ? 'tcp:4791' : null, reader && !present); let child;
    try {
      child = androidDeploy(fixture.app, fixture.bin, fixture.env).spawn();
      const closed = once(child, 'close');
      let out = '', error = '';
      child.stdout.on('data', data => { out += data; });
      child.stderr.on('data', data => { error += data; });
      await once(child.stdout, 'data');
      child.stdin.end('{"op":"quit"}\n');
      expect(await closed).toEqual([0, null]);
      expect(error).toBe(''); expect(out.trim()).toBe('{"ready":true}');
      const commands = fixture.commands();
      expect(commands.every(args => args[0] === '-s' && args[1] === 'fixture-device')).toBe(true);
      const reverse = commands.map(args => args.slice(2)).filter(args => args[0] === 'reverse');
      expect(reverse).toEqual(present ? [['reverse', '--list']] : [
        ['reverse', '--list'], ['reverse', '--no-rebind', 'tcp:4791', 'tcp:4791'], ['reverse', '--list'], ['reverse', '--remove', 'tcp:4791'],
      ]);
      expect(fixture.mappings()).toEqual(present ? { 'tcp:4791': 'tcp:4791' } : reader ? { 'tcp:4791': 'tcp:9000' } : {});
      const launch = commands.find(args => args[2] === 'shell' && (reader ? args[3].startsWith('am start ') : args[3] === '-T'));
      expect(launch.at(-1)).toContain(reader ? "--es x_auth_fixture 'http://127.0.0.1:4791'" : "EXACT_AUTH_FIXTURE='http://127.0.0.1:4791'");
    } finally {
      if (child && child.exitCode === null) { const closed = once(child, 'close'); child.kill(); await closed; }
      fixture.dispose();
    }
  });
for (const reader of [false, true]) test(`the Android ${reader ? 'reader' : 'headless'} drive refuses a conflicting reverse without changing it`, async () => {
  const { androidDeploy } = await import('./agent-android.mjs');
  const fixture = androidDriveFixture(reader, 'tcp:9000');
  try {
    expect(() => androidDeploy(fixture.app, fixture.bin, fixture.env)).toThrow('tcp:4791 maps to tcp:9000; needed tcp:4791');
    expect(fixture.commands().map(args => args.slice(2)).filter(args => args[0] === 'reverse')).toEqual([['reverse', '--list']]);
    expect(fixture.mappings()).toEqual({ 'tcp:4791': 'tcp:9000' });
  } finally { fixture.dispose(); }
});
for (const killed of ['parent', 'relay', 'parent before connect']) test(`the Android reader removes its reverse when the ${killed} is killed`, async () => {
  const fixture = androidDriveFixture(true), early = killed === 'parent before connect';
  if (early) fixture.stopSocket();
  let driver, relayPid;
  try {
    driver = spawn(process.execPath, ['--eval', `
      import { androidDeploy } from ${JSON.stringify(resolve(import.meta.dir, 'agent-android.mjs'))};
      const child = androidDeploy(${JSON.stringify(fixture.app)}, ${JSON.stringify(fixture.bin)}, process.env).spawn();
      child.stderr.pipe(process.stderr);
      if (${early}) console.log(JSON.stringify({ relay: child.pid }));
      else child.stdout.once('data', () => console.log(JSON.stringify({ relay: child.pid })));
      child.on('close', () => process.exit(0));
      process.stdin.on('data', () => child.kill('SIGKILL'));
    `], { env: fixture.env, stdio: ['pipe', 'pipe', 'pipe'] });
    const closed = once(driver, 'close');
    const [ready] = await once(driver.stdout, 'data'); relayPid = JSON.parse(ready).relay;
    if (early) await pollUntil(() => fixture.commands().some(args => args[2] === 'forward'), Boolean, { bound: 10000 });
    if (killed.startsWith('parent')) driver.kill('SIGKILL'); else driver.stdin.write('kill relay');
    await closed;
    if (early) await pollUntil(() => Object.keys(fixture.mappings()).length === 0, Boolean, { bound: 8000 });
    else await fixture.closed; // The relay's process has closed its device socket.
    const reverse = fixture.commands().map(args => args.slice(2)).filter(args => args[0] === 'reverse');
    expect(reverse).toEqual([
      ['reverse', '--list'], ['reverse', '--no-rebind', 'tcp:4791', 'tcp:4791'], ['reverse', '--list'], ['reverse', '--remove', 'tcp:4791'],
    ]);
    expect(fixture.mappings()).toEqual({});
  } finally {
    if (driver && driver.exitCode === null && driver.signalCode === null) { const closed = once(driver, 'close'); driver.kill(); await closed; }
    if (relayPid) { try { process.kill(relayPid, 'SIGKILL'); } catch {} }
    fixture.dispose();
  }
}, 15000);

for (const eof of [false, true]) test(`the Android reader cleans up before connecting on ${eof ? 'stdin EOF' : 'connect timeout'}`, async () => {
  const { androidDeploy } = await import('./agent-android.mjs');
  const fixture = androidDriveFixture(true); let child;
  fixture.stopSocket();
  try {
    child = androidDeploy(fixture.app, fixture.bin, fixture.env).spawn();
    const closed = once(child, 'close'); let error = '';
    child.stderr.on('data', data => { error += data; });
    if (eof) { await pollUntil(() => fixture.commands().some(args => args[2] === 'forward'), Boolean, { bound: 10000 }); child.stdin.end(); }
    const [code] = await closed;
    expect(code === 0).toBe(eof);
    if (!eof) expect(error).toContain('the reader opened no agent socket');
    expect(fixture.mappings()).toEqual({});
    expect(fixture.commands().filter(args => args[2] === 'reverse' && args[3] === '--remove')).toHaveLength(1);
    const commands = fixture.commands().map(args => args.slice(2)), forwarded = commands.findIndex(args => args[0] === 'forward' && args[1] === 'tcp:0');
    expect(forwarded).toBeGreaterThan(-1);
    expect(commands.slice(forwarded + 1)).toContainEqual(['forward', '--remove', `tcp:${fixture.port}`]);
    expect(commands.slice(forwarded + 1)).toContainEqual(['shell', 'am', 'force-stop', fixture.app.id]);
  } finally {
    if (child && child.exitCode === null) { const closed = once(child, 'close'); child.kill(); await closed; }
    fixture.dispose();
  }
}, 15000);

for (const failure of ['retained', 'replaced']) test(`the Android reader retries only its remaining reverse after a failed removal (${failure})`, async () => {
  const { androidDeploy } = await import('./agent-android.mjs');
  const fixture = androidDriveFixture(true, null, true, failure); let child;
  fixture.env.EXACT_DEV_PLAN = 'http://127.0.0.1:4792';
  try {
    child = androidDeploy(fixture.app, fixture.bin, fixture.env).spawn();
    const closed = once(child, 'close');
    await once(child.stdout, 'data'); child.stdin.end('{"op":"quit"}\n');
    expect(await closed).toEqual([1, null]);
    expect(fixture.mappings()).toEqual({ 'tcp:4791': 'tcp:9000', ...(failure === 'replaced' ? { 'tcp:4792': 'tcp:9001' } : {}) });
    const removals = fixture.commands().filter(args => args[2] === 'reverse' && args[3] === '--remove').map(args => args[4]);
    expect(removals.filter(port => port === 'tcp:4791')).toHaveLength(1);
    expect(removals.filter(port => port === 'tcp:4792')).toHaveLength(failure === 'retained' ? 2 : 1);
  } finally {
    if (child && child.exitCode === null) { const closed = once(child, 'close'); child.kill(); await closed; }
    fixture.dispose();
  }
});
