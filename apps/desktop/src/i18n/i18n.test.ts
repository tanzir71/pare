import { afterEach, describe, expect, it } from "vitest";
import { en } from "./en";
import { es } from "./es";
import { getLocale, resolveLocale, setLocale, t, tp } from "./index";
import { ago, formatBytes, formatBytesExact, formatCount } from "../lib/format";

const CATALOGS = { en, es } as const;

function placeholders(s: string): string[] {
  return [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]!).sort();
}

afterEach(() => setLocale("en"));

describe("catalogs", () => {
  it("have the same keys in English and Spanish", () => {
    const enKeys = Object.keys(en).sort();
    const esKeys = Object.keys(es).sort();
    expect(esKeys.filter((k) => !(k in en))).toEqual([]);
    expect(enKeys.filter((k) => !(k in es))).toEqual([]);
  });

  it("use the same placeholders per key", () => {
    const mismatched = Object.keys(en).filter((k) => {
      const key = k as keyof typeof en;
      return placeholders(en[key]).join(",") !== placeholders(es[key]).join(",");
    });
    expect(mismatched).toEqual([]);
  });

  it("have no empty messages", () => {
    for (const [lang, catalog] of Object.entries(CATALOGS)) {
      const empty = Object.entries(catalog).filter(([, v]) => v.trim() === "").map(([k]) => `${lang}:${k}`);
      expect(empty).toEqual([]);
    }
  });

  it("define plural messages as complete one/other pairs", () => {
    const keys = Object.keys(en);
    const broken = keys.filter(
      (k) =>
        (k.endsWith(".one") && !keys.includes(`${k.slice(0, -4)}.other`)) ||
        (k.endsWith(".other") && !keys.includes(`${k.slice(0, -6)}.one`)),
    );
    expect(broken).toEqual([]);
  });

  it("only use well-formed placeholders", () => {
    for (const catalog of Object.values(CATALOGS)) {
      for (const v of Object.values(catalog)) {
        const opens = (v.match(/\{/g) ?? []).length;
        expect(opens).toBe(placeholders(v).length);
      }
    }
  });
});

describe("resolveLocale", () => {
  it("uses the setting when one is chosen", () => {
    expect(resolveLocale("es", "en-US")).toBe("es");
    expect(resolveLocale("es-MX", "en-US")).toBe("es");
    expect(resolveLocale("ES", "en-US")).toBe("es");
    expect(resolveLocale("en", "es-ES")).toBe("en");
  });

  it("follows Windows when the setting is empty", () => {
    expect(resolveLocale("", "es-419")).toBe("es");
    expect(resolveLocale("  ", "es")).toBe("es");
    expect(resolveLocale("", "en-GB")).toBe("en");
    expect(resolveLocale("", "")).toBe("en");
  });

  it("falls back to English for languages without a catalog", () => {
    expect(resolveLocale("fr")).toBe("en");
    expect(resolveLocale("", "de-DE")).toBe("en");
    expect(resolveLocale("et", "et-EE")).toBe("en");
  });
});

describe("t and tp", () => {
  it("fill placeholders and switch language", () => {
    expect(t("common.freeUp", { size: "2.4 GB" })).toBe("Free up 2.4 GB");
    setLocale("es");
    expect(getLocale()).toBe("es");
    expect(document.documentElement.lang).toBe("es");
    expect(t("common.freeUp", { size: "2,4 GB" })).toBe("Liberar 2,4 GB");
  });

  it("pick plural forms with Intl.PluralRules", () => {
    expect(tp("count.items", 1)).toBe("1 item");
    expect(tp("count.items", 0)).toBe("0 items");
    expect(tp("count.items", 12345)).toBe("12,345 items");
    setLocale("es");
    expect(tp("count.items", 1)).toBe("1 elemento");
    expect(tp("count.items", 3)).toBe("3 elementos");
    expect(tp("count.items", 12345)).toBe("12.345 elementos");
    expect(tp("count.items", 1_000_000)).toBe("1.000.000 elementos");
  });

  it("format numbers, bytes and times for the active locale", () => {
    expect(formatBytes(2.4e9)).toBe("2.4 GB");
    expect(formatBytesExact(1234567)).toBe("1,234,567 bytes");
    expect(formatCount(1912)).toBe("1,912");
    const now = Date.now();
    expect(ago(now - 5 * 60_000, now)).toBe("5 mins ago");
    setLocale("es");
    expect(formatBytes(2.4e9)).toBe("2,4 GB");
    expect(formatBytes(412e9)).toBe("412 GB");
    expect(formatBytesExact(1234567)).toBe("1.234.567 bytes");
    expect(ago(now - 5 * 60_000, now)).toBe("hace 5 min");
    expect(ago(now - 240 * 86_400_000, now)).toBe("hace 8 meses");
  });
});
