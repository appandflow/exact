import { test } from 'bun:test';
import assert from 'node:assert/strict';

const flat = (s) => s.replace(/\s+/g, '');
const base = { id: 'com.exact.test', displayName: 'Test', manifest: { host: { ios: { associatedDomains: ['applinks:exact.test', 'webcredentials:exact.test'] } } } };
const app = { ...base, manifest: { host: { ios: { ...base.manifest.host.ios, appClip: {} } } } };
const grant = 'com.apple.developer.associated-appclip-app-identifiers';

test('a signed App Clip is On Demand Install Capable; its simulator build and its parent are not', async () => {
  const { appClip, entitlements } = await import('../host/apple/build.mjs');
  const clip = { ...app, id: appClip(app).id, clipParent: app.id };
  assert.match(flat(entitlements(clip, 'TEAM', true, null, { prefix: 'PREFIX' })), /<key>com.apple.developer.on-demand-install-capable<\/key><true\/>/);
  assert.doesNotMatch(entitlements(clip, null, true, null, { simulator: true }), /on-demand-install-capable/);
  assert.doesNotMatch(entitlements(app, 'TEAM', true), /on-demand-install-capable/);
});

test('a parent names its App Clip only when its profile grants that', async () => {
  const { appClip, entitlements } = await import('../host/apple/build.mjs');
  const clip = { ...app, id: appClip(app).id, clipParent: app.id };
  assert.doesNotMatch(entitlements(app, 'TEAM', true), /associated-appclip-app-identifiers/);
  assert.match(flat(entitlements(app, 'TEAM', true, null, { prefix: 'PREFIX', allows: [grant] })), /<key>com.apple.developer.associated-appclip-app-identifiers<\/key><array><string>PREFIX.com.exact.test.Clip<\/string><\/array>/);
  assert.doesNotMatch(entitlements(clip, 'TEAM', true, null, { prefix: 'PREFIX', allows: [grant] }), /associated-appclip-app-identifiers/);
  assert.doesNotMatch(entitlements(base, 'TEAM', true, null, { allows: [grant] }), /associated-appclip-app-identifiers/);
});
