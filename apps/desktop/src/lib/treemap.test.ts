import { describe, expect, it } from "vitest";
import { squarify } from "./treemap";

describe("squarify", () => {
  const rect = { x: 0, y: 0, w: 600, h: 400 };
  const items = [6, 6, 4, 3, 2, 2, 1].map((v, i) => ({ id: `n${i}`, value: v }));
  const out = squarify(items, rect);

  it("areas are proportional to values and fill the rectangle", () => {
    const total = items.reduce((a, i) => a + i.value, 0);
    for (const r of out) {
      expect(r.w * r.h).toBeCloseTo((r.value / total) * rect.w * rect.h, 3);
    }
    const area = out.reduce((a, r) => a + r.w * r.h, 0);
    expect(area).toBeCloseTo(rect.w * rect.h, 3);
  });

  it("stays inside the rectangle without overlaps", () => {
    for (const r of out) {
      expect(r.x).toBeGreaterThanOrEqual(-1e-9);
      expect(r.y).toBeGreaterThanOrEqual(-1e-9);
      expect(r.x + r.w).toBeLessThanOrEqual(rect.w + 1e-6);
      expect(r.y + r.h).toBeLessThanOrEqual(rect.h + 1e-6);
    }
    for (let i = 0; i < out.length; i++) {
      for (let j = i + 1; j < out.length; j++) {
        const a = out[i]!;
        const b = out[j]!;
        const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
        const oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
        expect(ox > 1e-6 && oy > 1e-6).toBe(false);
      }
    }
  });

  it("keeps aspect ratios reasonable", () => {
    const worst = Math.max(...out.map((r) => Math.max(r.w / r.h, r.h / r.w)));
    expect(worst).toBeLessThan(4);
  });

  it("handles zero values and empty input", () => {
    const z = squarify([{ id: "a", value: 0 }], rect);
    expect(z[0]).toMatchObject({ w: 0, h: 0 });
    expect(squarify([], rect)).toEqual([]);
  });
});
