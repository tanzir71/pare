// Copy formatting (DESIGN §1 Type; PRD §7 Voice). Decimal units, tabular numbers.
import type { ComesBack, ProtectReason, Tier, Timestamp } from "./types";

export { formatBytes, formatBytesExact } from "@pare/ui";

export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}

const DAY = 86_400_000;

export function durationPhrase(ms: number): string {
  const days = Math.max(0, Math.floor(ms / DAY));
  if (days < 1) {
    const hours = Math.floor(ms / 3_600_000);
    return hours < 1 ? "less than an hour" : plural(hours, "hour");
  }
  if (days < 45) return plural(days, "day");
  if (days < 365) return plural(Math.round(days / 30.44), "month");
  return plural(Math.floor(days / 365.25), "year");
}

export function ago(t: Timestamp | null | undefined, now = Date.now()): string {
  if (!t) return "—";
  const d = now - t;
  if (d < 0) return "just now";
  if (d < 60_000) return "just now";
  if (d < DAY) {
    const mins = Math.floor(d / 60_000);
    if (mins < 60) return `${plural(mins, "min")} ago`;
    return `${plural(Math.floor(mins / 60), "hour")} ago`;
  }
  return `${durationPhrase(d)} ago`;
}

export function date(t: Timestamp | null | undefined): string {
  if (!t) return "—";
  return new Date(t).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
}

export function dateTime(t: Timestamp | null | undefined): string {
  if (!t) return "—";
  return new Date(t).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function comesBackLabel(c: ComesBack): string {
  return c === "automatically" ? "Yes, automatically" : c === "on-demand" ? "Re-downloads when needed" : "No";
}

export function protectLabel(r: ProtectReason): string {
  switch (r) {
    case "in-use":
    case "tool-running":
      return "In use";
    case "uncommitted":
      return "Uncommitted";
    case "recently-modified":
      return "Recent";
    case "cloud-only":
      return "Cloud only";
    default:
      return "Keep";
  }
}

export function protectExplain(r: ProtectReason): string {
  switch (r) {
    case "recently-modified":
      return "Changed in the last 24 hours";
    case "in-use":
      return "Open in another app";
    case "tool-running":
      return "The app that owns it is running";
    case "uncommitted":
      return "Part of uncommitted work in a git repository";
    case "system-path":
      return "A Windows or system location Pare never touches";
    case "cloud-only":
      return "Stored in the cloud; removing it locally frees nothing";
    case "excluded":
      return "You chose to always keep this";
    case "changed-since-scan":
      return "Changed since the scan";
    case "never-touch":
      return "Settings or credentials Pare never touches";
  }
}

export type BadgeTone = "safe" | "review" | "protected" | "neutral" | "accent";

export function tierBadge(t: Tier, custom?: string | null): { tone: BadgeTone; label: string; sr: string } {
  if (t.kind === "protected") {
    return { tone: "protected", label: protectLabel(t.reason), sr: `Protected: ${t.detail ?? protectExplain(t.reason)}` };
  }
  if (t.kind === "safe") return { tone: "safe", label: custom ?? "Safe", sr: "Safe: comes back or isn't needed" };
  return { tone: "review", label: custom ?? "Review", sr: "Review: your data, check before removing" };
}
