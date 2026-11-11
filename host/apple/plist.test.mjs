// What a package's Apple implementation adds to Info.plist: `"plistKeys"` against the allow-list, `"plist"`
// dictionaries, and the merge into `infoPlist` and `macInfoPlist`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { PLIST_KEYS, modulePlist, packageKeys } from './plist.mjs';
import { infoPlist, macInfoPlist } from './build.mjs';

const packages = resolve(import.meta.dir, '../../packages');
const purchases = JSON.parse(readFileSync(resolve(packages, 'purchases/apple/exact.json'), 'utf8'));
const impl = (plistKeys) => [{ extension: 'purchases', plistKeys }];
const flat = (plist) => plist.replace(/>\s+</g, '><');
const KEY = /<key>SKIncludeConsumableInAppPurchaseHistory<\/key><true\/>/;

/** An app in a temporary directory whose `app.ts` is `source`, with `names` (directories under packages/) linked
 * into its node_modules, or copied when `edit` changes their Apple `exact.json`. */
function withApp(source, names, run, edit = null) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-plist-keys-'));
  try {
    mkdirSync(resolve(dir, 'node_modules/@exact'), { recursive: true });
    for (const name of names) {
      const at = resolve(dir, 'node_modules/@exact', name);
      if (!edit) { symlinkSync(resolve(packages, name), at, 'dir'); continue; }
      cpSync(resolve(packages, name), at, { recursive: true });
      const json = resolve(at, 'apple/exact.json');
      writeFileSync(json, JSON.stringify(edit(JSON.parse(readFileSync(json, 'utf8')))));
    }
    writeFileSync(resolve(dir, 'app.ts'), source);
    mkdirSync(resolve(dir, 'strings'));
    writeFileSync(resolve(dir, 'strings/en.json'), JSON.stringify({ nav: 'Turn-by-turn directions need your precise location.' }));
    return run({ id: 'com.example.plist', displayName: 'Plist', dir, manifest: { host: {} } });
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test("purchases' key is allowed on each platform the list names", () => {
  assert.deepEqual(purchases.plistKeys, { SKIncludeConsumableInAppPurchaseHistory: true });
  for (const target of ['ios', 'macos', 'tvos']) assert.deepEqual(packageKeys(impl(purchases.plistKeys), target), [{ extension: 'purchases', key: 'SKIncludeConsumableInAppPurchaseHistory', value: true }]);
  assert.deepEqual(packageKeys([{ extension: 'purchases', plistKeys: null }], 'ios'), []);
});

test('the allow-list refuses, naming the package, the key and what is allowed', () => {
  const refuses = (plistKeys, what) => assert.throws(() => packageKeys(impl(plistKeys), 'ios'),
    (e) => e.message.includes("purchases's Apple implementation") && e.message.includes(what) && e.message.includes('SKIncludeConsumableInAppPurchaseHistory: true'));
  refuses({ NSAppTransportSecurity: { NSAllowsArbitraryLoads: true } }, 'names NSAppTransportSecurity, which is not on the list');
  refuses({ CFBundleIdentifier: 'com.example.other' }, 'names CFBundleIdentifier');
  refuses({ SKIncludeConsumableInAppPurchaseHistory: false }, 'gives SKIncludeConsumableInAppPurchaseHistory = false');
  refuses({ SKIncludeConsumableInAppPurchaseHistory: 'YES' }, 'gives SKIncludeConsumableInAppPurchaseHistory = "YES"');
  refuses([], 'is not an object');
});

test('a package key that collides with a key the build writes is refused, naming the package', () => {
  const app = { id: 'com.example.plist', displayName: 'Plist', manifest: { host: {} } };
  const collides = (reach) => {
    assert.throws(() => infoPlist(app, false, { reach }), /geolocation's Apple implementation declares the Info.plist key CFBundleIdentifier, which the build writes/);
    assert.throws(() => macInfoPlist(app, { reach }), /geolocation's Apple implementation declares the Info.plist key CFBundleIdentifier/);
  };
  collides({ packages: [{ extension: 'geolocation', key: 'CFBundleIdentifier', value: {} }] });
  // A listed key the build also wrote would be refused the same way.
  PLIST_KEYS.set('UILaunchScreen', { value: true, platforms: ['ios'] });
  try {
    const reach = { packages: packageKeys([{ extension: 'purchases', plistKeys: { UILaunchScreen: true } }], 'ios') };
    assert.throws(() => infoPlist(app, false, { reach }), /purchases's Apple implementation declares the Info.plist key UILaunchScreen, which the build writes/);
  } finally { PLIST_KEYS.delete('UILaunchScreen'); }
});

test('an app that grants no device still gets its packages\' keys and dictionaries, and only from packages it uses', () => {
  withApp('import { purchase } from "@exact/purchases";\n', ['purchases'], (app) => {
    for (const plist of [infoPlist(app, false, { reach: modulePlist(app, null, 'ios') }), macInfoPlist(app, { reach: modulePlist(app, null, 'macos') })]) {
      assert.match(flat(plist), KEY);
      assert.doesNotMatch(plist, /UsageDescription|CFBundleLocalizations/);
    }
    // Purchases has no tvOS implementation.
    assert.doesNotMatch(infoPlist(app, false, { reach: modulePlist(app, null, 'tvos'), tv: true }), /SKInclude/);
  });
  withApp('export const nothing = true;\n', ['purchases'], (app) => assert.doesNotMatch(infoPlist(app, false, { reach: modulePlist(app, null, 'ios') }), /SKInclude/));
  withApp('import { requestPreciseLocation } from "@exact/geolocation";\nrequestPreciseLocation({ purposeKey: "nav" });\n', ['geolocation'], (app) => {
    assert.match(flat(infoPlist(app, false, { reach: modulePlist(app, null, 'ios') })),
      /<key>NSLocationTemporaryUsageDescriptionDictionary<\/key><dict><key>nav<\/key><string>Turn-by-turn directions need your precise location.<\/string><\/dict>/);
  });
});

test('a plist entry that is not "<call>.<option>" is refused, naming the package and the field', () => {
  for (const plist of [{ SKIncludeConsumableInAppPurchaseHistory: true }, { NSLocationTemporaryUsageDescriptionDictionary: 'requestPreciseLocation' }, ['x']]) {
    withApp('import { purchase } from "@exact/purchases";\n', ['purchases'], (app) => {
      assert.throws(() => modulePlist(app, null, 'ios'), /purchases's Apple implementation: its plist (gives|is not an object)/);
    }, (json) => ({ ...json, plist }));
  }
});
