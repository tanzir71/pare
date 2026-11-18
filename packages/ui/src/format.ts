// Byte formatting that matches what Explorer users expect (DESIGN.md §1 Type):
// decimal units (1 GB = 10^9 bytes), one decimal under 100 ("12.8 GB"), none above ("412 GB").

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** Format a byte count for display: `formatBytes(12_800_000_000) === "12.8 GB"`. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  let unit = 0;
  let value = bytes;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return `${Math.round(value)} B`;

  // Round first, then re-check the boundaries: 99.96 GB reads "100 GB" and 999.6 MB reads "1.0 GB".
  let rounded = value < 100 ? Math.round(value * 10) / 10 : Math.round(value);
  if (rounded >= 1000 && unit < UNITS.length - 1) {
    unit += 1;
    rounded = Math.round((rounded / 1000) * 10) / 10;
  }
  const text = rounded < 100 ? rounded.toFixed(1) : String(rounded);
  return `${text} ${UNITS[unit]}`;
}

/** Exact byte count for tooltips: `formatBytesExact(1234567) === "1,234,567 bytes"`. */
export function formatBytesExact(bytes: number): string {
  const n = Math.max(0, Math.round(bytes));
  return `${n.toLocaleString("en-US")} ${n === 1 ? "byte" : "bytes"}`;
}
