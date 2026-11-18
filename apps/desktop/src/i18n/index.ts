// UI localization. Catalogs are flat objects of message keys (`en.ts` is the source of truth);
// `{name}` placeholders are filled from `vars`, numbers formatted for the active locale.
// Plurals use `key.one` / `key.other` (plus any other CLDR category a catalog defines), picked
// with Intl.PluralRules. Text that comes from the backend (finding titles, why/ifRemoved, guide
// steps) arrives already translated after `locale_set`; it is never looked up here.
import { useSyncExternalStore } from "react";
import { en, type MessageKey } from "./en";
import { es } from "./es";

export type { MessageKey };

export type Locale = "en" | "es";
export const LOCALES: readonly Locale[] = ["en", "es"];

const CATALOGS: Record<Locale, Readonly<Record<MessageKey, string>>> = { en, es };

/** BCP 47 tags handed to Intl. English keeps the US formats Pare has always used. */
const INTL_TAG: Record<Locale, string> = { en: "en-US", es: "es" };

/** Keys that come in plural forms: `base.one` + `base.other` → `base`. */
export type PluralKey = {
  [K in MessageKey]: K extends `${infer B}.other` ? (`${B}.one` extends MessageKey ? B : never) : never;
}[MessageKey];

export type Vars = Readonly<Record<string, string | number>>;

function systemLanguage(): string {
  return typeof navigator !== "undefined" ? (navigator.language ?? "") : "";
}

/** The UI locale for a `language` setting: empty follows Windows (the WebView's language). */
export function resolveLocale(setting: string, system: string = systemLanguage()): Locale {
  const tag = (setting.trim() || system).trim().toLowerCase();
  return tag.startsWith("es") ? "es" : "en";
}

let current: Locale = resolveLocale("");
const listeners = new Set<() => void>();

export function getLocale(): Locale {
  return current;
}

/** Tag for Intl APIs (dates, numbers) in the active locale. */
export function intlLocale(locale: Locale = current): string {
  return INTL_TAG[locale];
}

/** Switch the UI language. Components using `useT()` re-render. */
export function setLocale(locale: Locale): void {
  if (typeof document !== "undefined") document.documentElement.lang = locale;
  if (locale === current) return;
  current = locale;
  for (const l of listeners) l();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

const numberFormats = new Map<Locale, Intl.NumberFormat>();
const pluralRules = new Map<Locale, Intl.PluralRules>();

/** `1234567` → "1,234,567" (en) or "1.234.567" (es). */
export function formatNumber(n: number, locale: Locale = current): string {
  let f = numberFormats.get(locale);
  if (!f) {
    f = new Intl.NumberFormat(INTL_TAG[locale]);
    numberFormats.set(locale, f);
  }
  return f.format(n);
}

function pluralCategory(n: number, locale: Locale): Intl.LDMLPluralRule {
  let r = pluralRules.get(locale);
  if (!r) {
    r = new Intl.PluralRules(INTL_TAG[locale]);
    pluralRules.set(locale, r);
  }
  return r.select(n);
}

/** Replace `{name}` placeholders; numbers are formatted for the locale, unknown names stay as is. */
export function fill(template: string, vars?: Vars, locale: Locale = current): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (m, name: string) => {
    const v = vars[name];
    if (v === undefined) return m;
    return typeof v === "number" ? formatNumber(v, locale) : v;
  });
}

function has(catalog: Readonly<Record<MessageKey, string>>, key: string): key is MessageKey {
  return Object.hasOwn(catalog, key);
}

/** Translate `key` in the active locale. */
export function t(key: MessageKey, vars?: Vars): string {
  return fill(CATALOGS[current][key], vars);
}

/** Translate a plural message: `tp("count.items", 3)` → "3 items". `{n}` is the formatted count. */
export function tp(key: PluralKey, n: number, vars?: Vars): string {
  const catalog = CATALOGS[current];
  const exact = `${key}.${pluralCategory(n, current)}`;
  const template = has(catalog, exact) ? catalog[exact] : catalog[`${key}.other`];
  return fill(template, { n, ...vars });
}

/** The active locale; re-renders the component when it changes. */
export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, getLocale, getLocale);
}

/** `const { t, tp } = useT();` — re-renders on language change. */
export function useT(): { t: typeof t; tp: typeof tp; locale: Locale } {
  const locale = useLocale();
  return { t, tp, locale };
}

/** For tests: the raw catalogs. */
export const catalogs: Readonly<Record<Locale, Readonly<Record<MessageKey, string>>>> = CATALOGS;
