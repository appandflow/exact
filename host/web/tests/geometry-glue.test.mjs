// LLP 1039 D9 on the wasm target: a resize carries the page's geometry (the
// safe-area insets, the segments, the keyboard's overlap) in its own
// `exact_resize` call, so a rotation's size and insets reach the runner as
// one commit, never the size with the insets before it.
import { expect, test } from 'bun:test';
import { hostFixtureFile as readFileSync, glueDeclarations } from './glue-fixture.mjs';
import vm from 'node:vm';

const source = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
const declarations = glueDeclarations(source, ['mediaChanged']);

function host(wasm) {
  const calls = [], written = [];
  const state = {
    innerWidth: 874, innerHeight: 402, incarnation: 1, JSON, document: { body: {} },
    readOut: value => value, applyBatch: batch => calls.push(['apply', batch]), presence: { resize: x => x },
    now: () => 0, preferences: () => 0, foldBits: () => 0, requestAnimationFrame: () => {}, positionContexts: () => {},
    pageChanged: () => calls.push(['page']),
    pageGeometry: () => '[[59,0,21,0],[],0]', writeIn: text => { written.push(text); return text.length; },
    wasm: { exact_resize: (...args) => { calls.push(['resize', ...args]); return '{}'; }, ...wasm },
  };
  state.host = state;
  vm.createContext(state);
  vm.runInContext(`${declarations}\nglobalThis.media = mediaChanged;`, state);
  return { state, calls, written };
}

test('a resize writes the geometry and hands its length to exact_resize', () => {
  const h = host({ exact_set_viewport_geometry: () => '{}' });
  h.state.media();
  expect(h.written).toEqual(['[[59,0,21,0],[],0]']);
  expect(h.calls).toEqual([['resize', 874, 402, 0, 0, 18], ['apply', {}], ['page']]);
});

test('a runner without the geometry export is resized as before', () => {
  const h = host({});
  h.state.media();
  expect(h.written).toEqual([]);
  expect(h.calls).toEqual([['resize', 874, 402, 0, 0, 0], ['apply', {}], ['page']]);
});
