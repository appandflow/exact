// Mosaic's Convex functions: a board of tiles, and the strokes that painted it.
// `paint` writes both in one transaction, so `get` and `recent` change together.
import { v } from 'convex/values';
import { mutation, query, type QueryCtx } from './_generated/server';
import { blank, paint as painted, RECENT, summary } from './tiles';

const named = (ctx: QueryCtx, name: string) => ctx.db.query('boards').withIndex('by_name', (q) => q.eq('name', name)).unique();

export const get = query({
  args: { name: v.string() },
  handler: async (ctx, { name }) => summary((await named(ctx, name)) ?? blank()),
});

export const recent = query({
  args: { name: v.string() },
  handler: async (ctx, { name }) =>
    (await ctx.db.query('strokes').withIndex('by_board', (q) => q.eq('board', name)).order('desc').take(RECENT))
      .map((s) => ({ id: s._id as string, by: s.by, index: s.index, hue: s.hue })),
});

export const paint = mutation({
  args: { name: v.string(), index: v.number(), hue: v.number(), by: v.string() },
  handler: async (ctx, { name, ...stroke }) => {
    const board = await named(ctx, name);
    const next = painted(board ?? blank(), stroke);
    if (board) await ctx.db.patch(board._id, next); else await ctx.db.insert('boards', { name, ...next });
    await ctx.db.insert('strokes', { board: name, ...stroke });
    return null;
  },
});

export const clear = mutation({
  args: { name: v.string() },
  handler: async (ctx, { name }) => {
    const board = await named(ctx, name);
    if (board) await ctx.db.delete(board._id);
    for (const s of await ctx.db.query('strokes').withIndex('by_board', (q) => q.eq('board', name)).collect()) await ctx.db.delete(s._id);
    return null;
  },
});
