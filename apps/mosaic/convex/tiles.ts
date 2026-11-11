// Mosaic's rules, with no Convex in them: board.ts runs them in Convex, and
// server.ts, the tests' stand-in, runs the same ones.

// Eight across, so a tile is a full touch target on a phone.
export const SIDE = 8;
export const TILES = SIDE * SIDE;
/** A tile's hue: 0 is unpainted, 1 to 6 the palette. */
export const HUES = 6;
/** How many of the newest strokes `recent` answers. */
export const RECENT = 5;

export type Board = { cells: number[]; strokes: number; painters: string[] };
export type Stroke = { by: string; index: number; hue: number };

export const blank = (): Board => ({ cells: Array.from({ length: TILES }, () => 0), strokes: 0, painters: [] });

/** The board after `by` paints tile `index` with `hue`; a tile or hue the board lacks is refused. */
export function paint(board: Board, { index, hue, by }: Stroke): Board {
  if (!Number.isInteger(index) || index < 0 || index >= TILES) throw new Error(`Mosaic has no tile ${index}.`);
  if (!Number.isInteger(hue) || hue < 0 || hue > HUES) throw new Error(`Mosaic has no hue ${hue}.`);
  const cells = board.cells.slice();
  cells[index] = hue;
  return { cells, strokes: board.strokes + 1, painters: board.painters.includes(by) ? board.painters : [...board.painters, by] };
}

/** What the `get` query answers. */
export const summary = (board: Board) => ({ cells: board.cells, strokes: board.strokes, painters: board.painters.length });
