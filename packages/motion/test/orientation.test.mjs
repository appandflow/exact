// `deviceOrientation`'s answer on Apple, from attitudes composed of W3C angles. The Apple implementation's
// conversion is compiled on this Mac with a main that reads quaternions and prints the angles it answers; Core
// Motion's sensors are the phone checklist's.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { conversions } from '../web/index.js';

const root = resolve(import.meta.dir, '../../..');
const MAIN = `@main struct Main {
  static func main() {
    let qs = try! JSONSerialization.jsonObject(with: Data(CommandLine.arguments[1].utf8)) as! [[Double]]
    let out = try! JSONSerialization.data(withJSONObject: qs.map { MotionModule.orientation($0) })
    print(String(decoding: out, as: UTF8.self))
  }
}
import Foundation
`;

/** The Apple implementation's angles for each quaternion of `qs`. */
function apple(qs) {
  mkdirSync(join(root, 'target'), { recursive: true });
  const out = mkdtempSync(join(root, 'target/motion-orientation-'));
  const run = (command, args) => {
    const r = spawnSync(command, args, { cwd: root, encoding: 'utf8', timeout: 120_000 });
    assert.equal(r.status, 0, `${command}: ${r.error ?? r.signal ?? ''}\n${r.stdout}\n${r.stderr}`);
    return r.stdout;
  };
  try {
    run('xcrun', ['swiftc', '-parse-as-library', '-emit-library', '-static', '-module-name', 'ExactPackage',
      '-emit-module', '-emit-module-path', join(out, 'ExactPackage.swiftmodule'),
      'host/apple/modules/ExactPackage.swift', '-o', join(out, 'libExactPackage.a')]);
    writeFileSync(join(out, 'main.swift'), MAIN);
    run('xcrun', ['swiftc', '-parse-as-library', '-I', out, '-L', out, '-lExactPackage',
      'packages/motion/apple/Motion.swift', join(out, 'main.swift'), '-o', join(out, 'orientation')]);
    return JSON.parse(run(join(out, 'orientation'), [JSON.stringify(qs)]));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

/** Whether `got` is `want` within 1e-6°, alpha taken round the circle. */
const same = (got, want) => got.every((g, i) => Math.abs(i ? g - want[i] : ((g - want[i] + 540) % 360) - 180) < 1e-6);

test.skipIf(process.platform !== 'darwin')('Apple answers deviceOrientation in the W3C ranges, as Android and the web', () => {
  // The issue's rows, which Core Motion's yaw, pitch and roll read as (−90, 0, 0), (−180, 60, −180), (−90, 80, −160).
  const rows = [[270, 0, 0], [0, 120, 0], [90, 100, 20]];
  // And attitudes over the whole of each range, off the boundaries where the angles are not unique.
  let seed = 7;
  const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  for (let i = 0; i < 200; i++) rows.push([random() * 360, -179 + random() * 358, -89 + random() * 178]);
  const got = apple(rows.map(([a, b, g]) => conversions.quaternionFromAngles(a, b, g)));
  const wrong = rows.map((want, i) => ({ want, got: got[i] })).filter(r => !same(r.got, r.want));
  assert.deepEqual(wrong, []);
  for (const [alpha, beta, gamma] of got) {
    assert.ok(alpha >= 0 && alpha < 360 && beta >= -180 && beta < 180 && gamma >= -90 && gamma < 90, `${alpha} ${beta} ${gamma}`);
  }
}, 240_000);
