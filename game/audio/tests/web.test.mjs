import { beforeAll, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';

const test = need('chrome', chromium().unavailable);
const game = resolve(import.meta.dir, '../..');
const fixture = resolve(game, 'target/audio-web-test');
beforeAll(() => {
  for (const [command, args] of [
    ['cargo', ['build', '--offline', '--locked', '-p', 'exact-game-audio', '--example', 'web_probe', '--target', 'wasm32-unknown-unknown']],
    ['wasm-bindgen', ['target/wasm32-unknown-unknown/debug/examples/web_probe.wasm', '--target', 'web', '--out-dir', fixture]],
  ]) {
    const result = spawnSync(command, args, { cwd: game, encoding: 'utf8' });
    if (result.error || result.status !== 0) throw new Error(`${command}: ${result.error ?? result.stderr}`);
  }
}, 180_000);

async function render(frames, updates) {
  const browser = await suiteBrowser();
  const page = await browser.newPage();
  try {
    await page.route('http://audio.test/**', route => {
      const path = new URL(route.request().url()).pathname;
      const type = path.endsWith('.wasm') ? 'application/wasm' : 'text/javascript';
      return route.fulfill(path === '/'
        ? { contentType: 'text/html', body: '<!doctype html>' }
        : { contentType: type, body: readFileSync(resolve(fixture, path.slice(1))) });
    });
    await page.goto('http://audio.test/');
    return await page.evaluate(async ({ frames, updates }) => {
      const context = new OfflineAudioContext(2, frames, 48000);
      // Keep the production graph and automation; substitute only the device
      // clock and activation so the rendered samples are deterministic.
      const suspend = context.suspend.bind(context), resume = context.resume.bind(context);
      context.suspend = context.resume = context.close = () => Promise.resolve();
      Object.defineProperty(context, 'state', { get: () => 'running' });
      window.AudioContext = function () { return context; };
      const { default: init, Probe } = await import('/web_probe.js');
      await init();
      const probe = new Probe();
      try {
        probe.unlock();
        // The next task observes the completed resume promise and wasm futures.
        await new Promise(resolve => {
          const task = new MessageChannel();
          task.port1.onmessage = () => { task.port1.close(); task.port2.close(); resolve(); };
          task.port2.postMessage(null);
        });
        if (!probe.ready()) throw new Error('output did not become ready');
        if (!probe.start(new Float32Array(frames).fill(1), 48000)) throw new Error('voice refused');
        const pending = updates.filter(u => u.frame > 0).map(update => ({
          update, reached: suspend(update.frame / 48000),
        }));
        for (const update of updates.filter(u => u.frame === 0)) probe.set(update.left, update.right);
        const rendered = context.startRendering();
        for (const { update, reached } of pending) {
          await reached;
          if (context.currentTime !== update.frame / 48000) throw new Error('wrong suspend frame');
          probe.set(update.left, update.right);
          await resume();
        }
        const buffer = await rendered;
        return [0, 1].map(channel => Array.from(buffer.getChannelData(channel)));
      } finally { probe.free(); }
    }, { frames, updates });
  } finally { await browser.close(); }
}

test('web voices reach their stereo gains linearly in ten milliseconds', async () => {
  const channels = await render(960, [{ frame: 0, left: 1, right: 0.5 }]);
  for (const [channel, target] of [1, 0.5].entries()) {
    const data = channels[channel];
    for (const frame of [0, 120, 240, 479, 480, 959]) {
      expect(data[frame]).toBeCloseTo(target * Math.min(frame / 480, 1), 5);
    }
    for (const length of [480, 960]) {
      const energy = data.slice(0, length).reduce((sum, value) => sum + value * value, 0);
      // Native advances its gain before each sample; Web Audio starts at zero.
      const nativeEnergy = Array.from({ length }, (_, i) => (target * Math.min((i + 1) / 480, 1)) ** 2)
        .reduce((sum, value) => sum + value, 0);
      expect(Math.abs(10 * Math.log10(energy / nativeEnergy))).toBeLessThan(0.03);
    }
  }
});

test('web gain retargeting holds the current value and replaces the old ramp', async () => {
  const updates = [
    { frame: 0, left: 0.25, right: 1 },
    { frame: 0, left: 1, right: 0.5 },
    { frame: 256, left: 0, right: 1 },
    { frame: 384, left: 0.75, right: 0.25 },
    { frame: 1024, left: 0, right: 0 },
  ];
  const channels = await render(1664, updates);
  for (const [channel, name] of ['left', 'right'].entries()) {
    let value = 0, step = 0, remaining = 0;
    for (let frame = 0; frame < channels[channel].length; frame++) {
      for (const update of updates.filter(u => u.frame === frame)) {
        step = (update[name] - value) / 480;
        remaining = 480;
      }
      expect(channels[channel][frame]).toBeCloseTo(value, 5);
      if (remaining > 0) { value += step; remaining--; }
    }
  }
});
