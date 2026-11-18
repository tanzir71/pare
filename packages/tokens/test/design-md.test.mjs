// Acceptance (M0): packages/tokens outputs CSS variables that match docs/DESIGN.md §1.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
const css = readFileSync(resolve(here, "../dist/tokens.css"), "utf8");
const design = readFileSync(resolve(root, "docs/DESIGN.md"), "utf8");

function block(selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing ${selector} block`);
  return css.slice(start, css.indexOf("}", start));
}
const lightCss = block(":root");
const darkCss = block('[data-theme="dark"]');

function varValue(blockText, name) {
  const m = blockText.match(new RegExp(`--pare-${name}:\\s*([^;]+);`));
  return m ? m[1].trim() : undefined;
}

// Parse the colour rows of DESIGN.md §1, e.g.
//   | `bg/app` | `#F6F7F9` | `#0A0F1A` | ... |
//   | `safe/fg` · `safe/bg` | `#067647` · `#ECFDF3` | `#47CD89` · `#0B2A1C` | ... |
const section = design.slice(design.indexOf("## 1. Tokens"), design.indexOf("### Type"));
const rows = [];
for (const line of section.split("\n")) {
  const cells = line.split("|").map((c) => c.trim());
  if (cells.length < 5 || !cells[1]?.startsWith("`")) continue;
  const names = [...cells[1].matchAll(/`([^`]+)`/g)].map((m) => m[1]);
  const lights = [...cells[2].matchAll(/`(#[0-9A-Fa-f]{6})`/g)].map((m) => m[1]);
  const darks = [...cells[3].matchAll(/`(#[0-9A-Fa-f]{6})`/g)].map((m) => m[1]);
  if (!lights.length || names.length !== lights.length) continue;
  names.forEach((n, i) => rows.push({ name: n.replace(/\//g, "-"), light: lights[i], dark: darks[i] }));
}

test("DESIGN.md §1 colour table parsed", () => {
  assert.ok(rows.length >= 20, `expected the full colour table, got ${rows.length} rows`);
});

for (const row of rows) {
  test(`--pare-${row.name} matches DESIGN.md (light ${row.light}, dark ${row.dark})`, () => {
    assert.equal(varValue(lightCss, row.name)?.toUpperCase(), row.light.toUpperCase());
    const dark = varValue(darkCss, row.name) ?? varValue(lightCss, row.name);
    assert.equal(dark?.toUpperCase(), row.dark.toUpperCase());
  });
}

test("radius and spacing scales match DESIGN.md", () => {
  for (const [k, v] of Object.entries({ sm: 6, md: 8, lg: 12, xl: 16 })) {
    assert.equal(varValue(lightCss, `radius-${k}`), `${v}px`);
  }
  for (const v of [4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 96]) {
    assert.match(lightCss, new RegExp(`--pare-space-\\d+: ${v}px;`));
  }
});

test("type scale matches DESIGN.md (H1 30/38, Body/M 14/20, Overline 11/16)", () => {
  assert.equal(varValue(lightCss, "type-heading-h1-size"), "30px");
  assert.equal(varValue(lightCss, "type-heading-h1-line"), "38px");
  assert.equal(varValue(lightCss, "type-body-m-size"), "14px");
  assert.equal(varValue(lightCss, "type-overline-size"), "11px");
});
