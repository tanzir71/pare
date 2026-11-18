// Squarified treemap layout (Bruls, Huizing & van Wijk): rows of rectangles whose aspect ratios
// stay close to 1, so sizes are easy to compare.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Weighted {
  value: number;
}

function worst(row: number[], side: number): number {
  const sum = row.reduce((a, b) => a + b, 0);
  const max = Math.max(...row);
  const min = Math.min(...row);
  const s2 = side * side;
  return Math.max((s2 * max) / (sum * sum), (sum * sum) / (s2 * min));
}

/** Lay out `items` (any order; values ≥ 0) inside `rect`. Zero-value items get empty rects. */
export function squarify<T extends Weighted>(items: readonly T[], rect: Rect): (T & Rect)[] {
  const out: (T & Rect)[] = [];
  const positive = items.filter((i) => i.value > 0).sort((a, b) => b.value - a.value);
  const total = positive.reduce((a, i) => a + i.value, 0);
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) {
    return items.map((i) => ({ ...i, x: rect.x, y: rect.y, w: 0, h: 0 }));
  }
  const scale = (rect.w * rect.h) / total;
  const areas = positive.map((i) => i.value * scale);
  let { x, y, w, h } = rect;
  let i = 0;
  while (i < positive.length) {
    const side = Math.min(w, h);
    const row: number[] = [areas[i]!];
    let j = i + 1;
    while (j < positive.length && worst([...row, areas[j]!], side) <= worst(row, side)) {
      row.push(areas[j]!);
      j += 1;
    }
    const sum = row.reduce((a, b) => a + b, 0);
    if (w >= h) {
      // Column on the left.
      const cw = sum / h;
      let cy = y;
      row.forEach((a, k) => {
        const ch = a / cw;
        out.push({ ...positive[i + k]!, x, y: cy, w: cw, h: ch });
        cy += ch;
      });
      x += cw;
      w -= cw;
    } else {
      // Row on top.
      const rh = sum / w;
      let cx = x;
      row.forEach((a, k) => {
        const rw = a / rh;
        out.push({ ...positive[i + k]!, x: cx, y, w: rw, h: rh });
        cx += rw;
      });
      y += rh;
      h -= rh;
    }
    i = j;
  }
  for (const it of items) if (!(it.value > 0)) out.push({ ...it, x: rect.x, y: rect.y, w: 0, h: 0 });
  return out;
}
