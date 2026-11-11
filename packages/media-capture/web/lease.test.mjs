import { expect, test } from 'bun:test';

// A lease given back and taken again while the device opens: two opens were in flight, both found the track active,
// and the second adopted its browser track over the first without stopping it, which then captured until the page
// closed (the camera's light, the browser's recording indicator). Each open adopts only for its own lease.
test('a lease retaken while the device opens leaves no browser track capturing', async () => {
  const made = [];
  const track = () => {
    const t = { label: 'Fake microphone', readyState: 'live', getSettings: () => ({}), stop() { this.readyState = 'ended'; } };
    made.push(t);
    return t;
  };
  const saved = globalThis.navigator;
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { mediaDevices: {
    getUserMedia: async () => { await Bun.sleep(50); const t = track(); return { getAudioTracks: () => [t], getTracks: () => [t] }; },
  } } });
  try {
    const { implementation } = await import('./index.js');
    const added = [];
    const impl = implementation({ changed() {}, agent: false, log() {}, tracks: { add: (row) => added.push(row), held: () => false } });
    const opened = await impl.getUserMedia({ audio: true });
    const id = opened.tracks[0].id, owner = added.find((row) => row.id === id).owner;
    // Used once, then idle: its browser track stops.
    owner.subscribe(id, {}, { start: true })();
    expect(made.filter((t) => t.readyState === 'live')).toEqual([]);
    // Start, leave, start again while the device opens; then the last consumer leaves.
    const first = owner.subscribe(id, {}, { start: true });
    first();
    const second = owner.subscribe(id, {}, { start: true });
    await Bun.sleep(150);
    second();
    expect(made.filter((t) => t.readyState === 'live').length).toBe(0);
  } finally {
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: saved });
  }
});
