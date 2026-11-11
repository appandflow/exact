import { expect, test } from 'bun:test';
import { roster } from '../web/index.js';

for (const [longitude, zoom, dx, dy, expected] of [
  [-122.404, 15, 0, 0, -122.404],
  [-122.404, 15, 80, 0, -122.40056677246094],
  [122.404, 15, -80, 0, 122.40056677246094],
  [-122.404, 15.8, 0, 80, -122.404],
  [179, 1, 8, 0, -175.375],
  [-179, 1, -8, 0, 175.375],
  [0, 1, 3 * 512, 0, 0],
  [0, 1, -3 * 512, 0, 0],
  [0, 1, 256, 0, -180],
]) {
  test(`pan from longitude ${longitude} at zoom ${zoom} by (${dx}, ${dy}) pixels reaches ${expected}`, () => {
    const map = Object.assign(Object.create(roster['exact-map'].view.prototype), {
      centre: [37.7946, longitude], zoom, draw() {},
    });
    map.panBy(dx, dy);
    expect(map.centre[1]).toBeCloseTo(expected, 9);
    if (dy === 0) expect(map.centre[0]).toBeCloseTo(37.7946, 9);
    expect(map.zoom).toBe(zoom);
  });
}
