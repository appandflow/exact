// The tests' stand-in for Mosaic's Convex deployment (`bun exact.mjs test` and
// `verify` start it; app.json's `commands.server`): convex/board.ts's four
// functions over the same rules (convex/tiles.ts), in memory, from an empty
// board. The functions themselves run only in Convex: `npx convex dev`.
import { serve } from '@exact/convex/test/server.ts';
import { blank, paint, RECENT, summary, type Board, type Stroke } from './convex/tiles.ts';

const boards = new Map<string, Board>(), strokes = new Map<string, (Stroke & { id: string })[]>();
let made = 0;

serve({
  port: Number(process.env.PORT ?? 3210),
  queries: {
    'board:get': ({ name }) => summary(boards.get(name) ?? blank()),
    'board:recent': ({ name }) => (strokes.get(name) ?? []).slice(-RECENT).reverse(),
  },
  mutations: {
    'board:paint': ({ name, ...stroke }) => {
      boards.set(name, paint(boards.get(name) ?? blank(), stroke));
      strokes.set(name, [...(strokes.get(name) ?? []), { id: `s${++made}`, ...stroke }]);
    },
    'board:clear': ({ name }) => { boards.delete(name); strokes.delete(name); },
  },
});
