import { normal, After } from "./rt.js";
// A colour's alpha and a `light-dark()`'s halves, as element.rs reads them: for a bound root background (`canvasColor`)
// and a button's bound fill (button-fill.js).
export const alpha = c => {
  const hex = /^#(?:[0-9a-f]{3}([0-9a-f])|[0-9a-f]{6}([0-9a-f]{2}))$/i.exec(c);
  if (hex) return hex[1] ? parseInt(hex[1], 16) / 15 : parseInt(hex[2], 16) / 255;
  const slash = /\/\s*(none|[\d.]+%?)\s*\)$/.exec(c);
  if (slash) return slash[1] === 'none' ? 0 : parseFloat(slash[1]) / (slash[1].endsWith('%') ? 100 : 1);
  const rgba = /^rgba\(.*,\s*([\d.]+)\s*\)$/.exec(c);
  return rgba ? parseFloat(rgba[1]) : c === 'transparent' ? 0 : 1;
};
export function halves(v) {
  const m = /^\s*light-dark\((.*)\)\s*$/i.exec(v); if (!m) return null;
  let depth = 0; for (let i = 0; i < m[1].length; i++) { const c = m[1][i]; if (c === '(') depth++; else if (c === ')') depth--; else if (c === ',' && !depth) return [m[1].slice(0, i), m[1].slice(i + 1)]; }
  return null;
}

/** The root's bound background `v` when the browser reads it as a colour opaque in both schemes, which makes it the
 * page's canvas (canvas.rs); else null, and the shell's canvas stays. A clear reads back as a CSS-wide keyword, which
 * names no colour. */
export function canvasColor(v) {
  const t = v == null ? "" : normal("background-color", String(v));
  themeFollows();
  return t && !/^\s*(unset|initial|inherit|revert|revert-layer)\s*$/i.test(t) && (halves(t) ?? [t]).every(h => alpha(h.trim()) >= 1) ? v : null;
}
/** A bound canvas is also the page's `theme-color`, which Chrome on Android, Safari's compact tab bar and an installed
 * app's title bar paint: the `html` element's computed background after each commit and each change of the system's
 * scheme, written over app.json's static ones (scripts/app-icons.mjs `webThemeColors`). */
let following = false;
function themeFollows() {
  if (following || typeof document === "undefined" || globalThis.__exactRender) return;
  following = true;
  const sync = () => {
    const colour = getComputedStyle(document.documentElement).backgroundColor;
    let metas = document.head.querySelectorAll('meta[name="theme-color"]');
    if (!metas.length) metas = [document.head.appendChild(Object.assign(document.createElement("meta"), { name: "theme-color" }))];
    for (const m of metas) if (m.content !== colour) m.content = colour;
  };
  After.push(sync);
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", sync);
}
