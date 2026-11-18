// Copy formatting (DESIGN §1 Type; PRD §7 Voice). Decimal units, tabular numbers.
// Everything here follows the active UI locale (i18n/index.ts) through Intl and the catalogs.
import { formatBytes as formatBytesBase } from "@pare/ui";
import { formatNumber, getLocale, intlLocale, t, tp, type Locale } from "../i18n";
import type { CategoryId, ComesBack, Growth, ProtectReason, Tier, Timestamp } from "./types";

const decimalSeparators = new Map<Locale, string>();

function decimalSeparator(): string {
  const locale = getLocale();
  let sep = decimalSeparators.get(locale);
  if (sep === undefined) {
    sep = new Intl.NumberFormat(intlLocale(locale)).formatToParts(1.5).find((p) => p.type === "decimal")?.value ?? ".";
    decimalSeparators.set(locale, sep);
  }
  return sep;
}

/** "12.8 GB" (en) or "12,8 GB" (es). Units are the same in every language. */
export function formatBytes(bytes: number): string {
  const s = formatBytesBase(bytes);
  const sep = decimalSeparator();
  return sep === "." ? s : s.replace(".", sep);
}

/** Exact byte count for tooltips: "1,234,567 bytes". */
export function formatBytesExact(bytes: number): string {
  return tp("count.bytes", Math.max(0, Math.round(bytes)));
}

export function formatCount(n: number): string {
  return formatNumber(n);
}

/** "a", "a and b", "a, b and c" in the active language. */
export function andList(items: string[]): string {
  if (items.length <= 1) return items[0] ?? "";
  return t("common.and", { a: items.slice(0, -1).join(", "), b: items.at(-1)! });
}

/** "a", "a or b", "a, b or c" in the active language. */
export function orList(items: string[]): string {
  if (items.length <= 1) return items[0] ?? "";
  return t("common.or", { a: items.slice(0, -1).join(", "), b: items.at(-1)! });
}

const DAY = 86_400_000;

export function durationPhrase(ms: number): string {
  const days = Math.max(0, Math.floor(ms / DAY));
  if (days < 1) {
    const hours = Math.floor(ms / 3_600_000);
    return hours < 1 ? t("time.lessThanHour") : tp("time.hours", hours);
  }
  if (days < 45) return tp("time.days", days);
  if (days < 365) return tp("time.months", Math.round(days / 30.44));
  return tp("time.years", Math.floor(days / 365.25));
}

export function ago(t0: Timestamp | null | undefined, now = Date.now()): string {
  if (!t0) return "—";
  const d = now - t0;
  if (d < 60_000) return t("time.justNow");
  if (d < DAY) {
    const mins = Math.floor(d / 60_000);
    if (mins < 60) return t("time.ago", { time: tp("time.mins", mins) });
    return t("time.ago", { time: tp("time.hours", Math.floor(mins / 60)) });
  }
  return t("time.ago", { time: durationPhrase(d) });
}

export function date(ts: Timestamp | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleDateString(intlLocale(), { month: "short", day: "numeric", year: "numeric" });
}

export function dateTime(ts: Timestamp | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString(intlLocale(), {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function comesBackLabel(c: ComesBack): string {
  return t(`comesBack.${c}`);
}

/** Category card title and description, by id (the backend's copies are English-only). */
export function categoryTitle(id: CategoryId): string {
  return t(`category.${id}.title`);
}

export function categoryDescription(id: CategoryId): string {
  return t(`category.${id}.description`);
}

export function protectLabel(r: ProtectReason): string {
  switch (r) {
    case "in-use":
    case "tool-running":
      return t("protect.label.inUse");
    case "uncommitted":
      return t("protect.label.uncommitted");
    case "recently-modified":
      return t("protect.label.recent");
    case "cloud-only":
      return t("protect.label.cloudOnly");
    default:
      return t("protect.label.keep");
  }
}

export function protectExplain(r: ProtectReason): string {
  return t(`protect.why.${r}`);
}

export type BadgeTone = "safe" | "review" | "protected" | "neutral" | "accent";

export function tierBadge(
  tier: Tier,
  custom?: string | null,
  disposal?: { kind: string },
): { tone: BadgeTone; label: string; sr: string } {
  if (disposal?.kind === "guide" && tier.kind !== "protected") {
    return { tone: "neutral", label: t("tier.guide"), sr: t("tier.guideSr") };
  }
  if (tier.kind === "protected") {
    return {
      tone: "protected",
      label: protectLabel(tier.reason),
      sr: t("tier.protectedSr", { detail: tier.detail ?? protectExplain(tier.reason) }),
    };
  }
  if (tier.kind === "safe") return { tone: "safe", label: custom ?? t("tier.safe"), sr: t("tier.safeSr") };
  return { tone: "review", label: custom ?? t("tier.review"), sr: t("tier.reviewSr") };
}

/** `C:\Users\ana\.codex` → `~\.codex` */
export function tildePath(p: string, home: string | undefined): string {
  if (home && p.toLowerCase().startsWith(home.toLowerCase())) return `~${p.slice(home.length)}`;
  return p;
}

/** "~\.codex grew 12 GB this week" */
export function growthPhrase(g: Growth, home: string | undefined, now = Date.now()): string {
  const what = g.path ? tildePath(g.path, home) : g.label;
  const days = Math.round((now - g.since) / 86_400_000);
  const size = formatBytes(Math.max(0, g.to - g.from));
  if (days >= 6) return t("growth.thisWeek", { what, size });
  if (days <= 1) return t("growth.sinceYesterday", { what, size });
  return t("growth.inDays", { what, size, days });
}
