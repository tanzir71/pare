// Generates the static brand assets in apps/web/public/ (run: pnpm --filter web assets):
//   favicon.svg           the approved mark (design/brand/pare-mark.svg without its C2PA metadata)
//   favicon.ico           16/32/48 PNG entries packed into one ICO
//   apple-touch-icon.png  180×180, mark on an opaque surface (iOS fills transparency with black)
//   og.png                1200×630 Open Graph / Twitter card
// Colours come from packages/tokens/dist/tokens.json. Text is converted to SVG paths with fontkit
// (static Geist instances), so the PNG does not depend on fonts installed on the machine.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import * as fontkit from "fontkit";
import sharp from "sharp";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const publicDir = (name) => here(`../public/${name}`);
const tokens = JSON.parse(readFileSync(here("../../../packages/tokens/dist/tokens.json"), "utf8"));
const color = (name) => {
  const value = tokens.color[name]?.light;
  if (!value) throw new Error(`Unknown colour token: ${name}`);
  return value;
};
const font = (weight) =>
  fontkit.openSync(
    here(`../node_modules/@fontsource/geist/files/geist-latin-${weight}-normal.woff2`),
  );
const fonts = { 400: font(400), 500: font(500), 600: font(600) };

// --- Mark -----------------------------------------------------------------------------------

const markSource = readFileSync(here("../../../design/brand/pare-mark.svg"), "utf8");
const markSvg = markSource
  .replace(/<metadata>[\s\S]*?<\/metadata>\s*/g, "")
  .replace(/\s+xmlns:c2pa="[^"]*"/, "")
  .replace(/\s*<!--[\s\S]*?-->/g, "")
  .replace(/\n\s*\n/g, "\n");
writeFileSync(publicDir("favicon.svg"), markSvg);

/** The mark's inner shapes, re-usable inside a larger SVG at any position and size. */
function markGroup(x, y, size, id = "mark") {
  const s = size / 64;
  return `
  <defs>
    <linearGradient id="${id}" x1="0" y1="0" x2="64" y2="64" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${color("brand-mark-gradient-start")}"/>
      <stop offset="1" stop-color="${color("brand-mark-gradient-end")}"/>
    </linearGradient>
  </defs>
  <g transform="translate(${x} ${y}) scale(${s})">
    <path d="M30 0H64V64H0V30A30 30 0 0 1 30 0Z" fill="url(#${id})"/>
    <path d="M30 16A17 17 0 1 0 47 33L30 33Z" fill="${color("brand-mark-disk")}"/>
    <path d="M35 11A17 17 0 0 1 52 28L35 28Z" fill="${color("brand-mark-slice")}"/>
  </g>`;
}

async function markPng(size) {
  // Render straight from vector at the target size for crisp small icons.
  return sharp(Buffer.from(markSvg), { density: (72 * size) / 64 })
    .resize(size, size)
    .png({ compressionLevel: 9 })
    .toBuffer();
}

/** Pack PNG images into a .ico (PNG-in-ICO is supported by Windows Vista+ and every browser). */
function packIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  const entries = [];
  let offset = 6 + images.length * 16;
  for (const { size, data } of images) {
    const e = Buffer.alloc(16);
    e.writeUInt8(size >= 256 ? 0 : size, 0);
    e.writeUInt8(size >= 256 ? 0 : size, 1);
    e.writeUInt8(0, 2);
    e.writeUInt8(0, 3);
    e.writeUInt16LE(1, 4);
    e.writeUInt16LE(32, 6);
    e.writeUInt32LE(data.length, 8);
    e.writeUInt32LE(offset, 12);
    offset += data.length;
    entries.push(e);
  }
  return Buffer.concat([header, ...entries, ...images.map((i) => i.data)]);
}

const icoImages = [];
for (const size of [16, 32, 48]) icoImages.push({ size, data: await markPng(size) });
writeFileSync(publicDir("favicon.ico"), packIco(icoImages));

const touchSize = 180;
const touchMark = 128;
const touchOffset = (touchSize - touchMark) / 2;
await sharp(
  Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${touchSize}" height="${touchSize}">
      <rect width="100%" height="100%" fill="${color("bg-surface")}"/>
      ${markGroup(touchOffset, touchOffset, touchMark)}
    </svg>`,
  ),
)
  .png({ compressionLevel: 9 })
  .toFile(publicDir("apple-touch-icon.png"));

// --- Open Graph image ----------------------------------------------------------------------

/** Lay out a single line of text as SVG paths. Returns the markup and the advance width. */
function text(str, { x, y, size, weight = 400, fill, tracking = 0 }) {
  const f = fonts[weight];
  const run = f.layout(str);
  const scale = size / f.unitsPerEm;
  let cursor = x;
  const paths = [];
  run.glyphs.forEach((glyph, i) => {
    const pos = run.positions[i];
    const d = glyph.path.toSVG();
    if (d) {
      const gx = (cursor + pos.xOffset * scale).toFixed(2);
      const gy = (y - pos.yOffset * scale).toFixed(2);
      paths.push(`<path transform="translate(${gx} ${gy}) scale(${scale} ${-scale})" d="${d}"/>`);
    }
    cursor += pos.xAdvance * scale + tracking * size;
  });
  return { svg: `<g fill="${fill}">${paths.join("")}</g>`, width: cursor - x - tracking * size };
}

const W = 1200;
const H = 630;
const left = 80;

const wordmark = text("Pare", {
  x: left + 48 + 15,
  y: 80 + 36,
  size: 36,
  weight: 600,
  fill: color("text-primary"),
  tracking: -0.03,
});
const line1 = text("Make room on your PC.", {
  x: left,
  y: 252,
  size: 76,
  weight: 600,
  fill: color("text-primary"),
  tracking: -0.035,
});
const line2 = text("Keep what matters.", {
  x: left,
  y: 336,
  size: 76,
  weight: 600,
  fill: color("text-primary"),
  tracking: -0.035,
});
const sub1 = text("A free, open-source cleanup app for Windows that shows", {
  x: left,
  y: 412,
  size: 28,
  fill: color("text-secondary"),
  tracking: -0.005,
});
const sub2 = text("what's safe to remove before anything is deleted.", {
  x: left,
  y: 452,
  size: 28,
  fill: color("text-secondary"),
  tracking: -0.005,
});

const pillText = text("Open source · Runs locally · No telemetry", {
  x: 0,
  y: 0,
  size: 22,
  weight: 500,
  fill: color("accent-on-subtle"),
});
const pill = { x: left, y: 500, h: 48, padL: 48, padR: 22 };
const pillW = pill.padL + pillText.width + pill.padR;
const pillLabel = text("Open source · Runs locally · No telemetry", {
  x: pill.x + pill.padL,
  y: pill.y + 32,
  size: 22,
  weight: 500,
  fill: color("accent-on-subtle"),
});
// lucide "shield-check" (24 px grid), drawn at 22 px.
const shield = `<g transform="translate(${pill.x + 16} ${pill.y + 13}) scale(${22 / 24})" fill="none" stroke="${color("accent-on-subtle")}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/>
  <path d="m9 12 2 2 4-4"/>
</g>`;

// The storage bar from the app, as a band along the bottom edge: Other used, Safe, Review, Free.
const bar = [
  ["accent-default", 371.4],
  ["safe-fg", 28],
  ["review-fg", 12.6],
  ["border-default", 100],
];
const barTotal = bar.reduce((sum, [, v]) => sum + v, 0);
let barX = 0;
const barSvg = bar
  .map(([token, v], i) => {
    const w = i === bar.length - 1 ? W - barX : Math.round((v / barTotal) * W);
    const rect = `<rect x="${barX}" y="${H - 16}" width="${w}" height="16" fill="${color(token)}"/>`;
    barX += w;
    return rect;
  })
  .join("");

const og = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <rect width="${W}" height="${H}" fill="${color("bg-surface")}"/>
  <circle cx="1030" cy="255" r="215" fill="${color("accent-subtle")}"/>
  ${markGroup(920, 145, 220, "og-mark")}
  ${markGroup(left, 80, 48, "og-lockup")}
  ${wordmark.svg}
  ${line1.svg}
  ${line2.svg}
  ${sub1.svg}
  ${sub2.svg}
  <rect x="${pill.x}" y="${pill.y}" width="${pillW.toFixed(1)}" height="${pill.h}" rx="${pill.h / 2}" fill="${color("accent-subtle")}"/>
  ${shield}
  ${pillLabel.svg}
  ${barSvg}
</svg>`;

if (line1.width > 880) throw new Error(`OG headline too wide (${line1.width.toFixed(0)} px)`);
await sharp(Buffer.from(og))
  .png({ compressionLevel: 9, palette: false })
  .toFile(publicDir("og.png"));

console.log(
  `favicon.svg, favicon.ico (16/32/48), apple-touch-icon.png (180), og.png (${W}×${H}) written to public/`,
);
