import { test } from "node:test";
import assert from "node:assert/strict";
import { formatBytes, formatBytesExact } from "../src/format.ts";

const GB = 1e9;

test("uses decimal units with one decimal under 100", () => {
  assert.equal(formatBytes(12.8 * GB), "12.8 GB");
  assert.equal(formatBytes(28 * GB), "28.0 GB");
  assert.equal(formatBytes(6.1 * GB), "6.1 GB");
  assert.equal(formatBytes(48 * 1e6), "48.0 MB");
});

test("drops the decimal at 100 and above", () => {
  assert.equal(formatBytes(412 * GB), "412 GB");
  assert.equal(formatBytes(100 * GB), "100 GB");
  assert.equal(formatBytes(371.4 * GB), "371 GB");
});

test("handles rounding across boundaries", () => {
  assert.equal(formatBytes(99.96 * GB), "100 GB");
  assert.equal(formatBytes(999.6 * 1e6), "1.0 GB");
  assert.equal(formatBytes(999_999), "1.0 MB");
});

test("formats bytes and edge cases", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(-5), "0 B");
  assert.equal(formatBytes(Number.NaN), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(2.5e12), "2.5 TB");
});

test("exact bytes for tooltips", () => {
  assert.equal(formatBytesExact(1234567), "1,234,567 bytes");
  assert.equal(formatBytesExact(1), "1 byte");
});
