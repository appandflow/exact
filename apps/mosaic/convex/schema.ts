import { defineSchema, defineTable } from 'convex/server';
import { v } from 'convex/values';

export default defineSchema({
  boards: defineTable({ name: v.string(), cells: v.array(v.number()), strokes: v.number(), painters: v.array(v.string()) }).index('by_name', ['name']),
  strokes: defineTable({ board: v.string(), by: v.string(), index: v.number(), hue: v.number() }).index('by_board', ['board']),
});
