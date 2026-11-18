// Review-list selection model (DESIGN §3 02 Review): click the checkbox or press Space to
// toggle; Shift-click selects a range; Ctrl+A selects all *selectable* rows; protected rows are
// skipped by every bulk selection.
import type { Finding } from "./types";

export type Selection = ReadonlySet<string>;

export function isSelectable(f: Finding): boolean {
  return f.tier.kind !== "protected" && f.disposal.kind !== "guide";
}

export function toggle(sel: Selection, f: Finding): Set<string> {
  const next = new Set(sel);
  if (!isSelectable(f)) return next;
  if (next.has(f.id)) next.delete(f.id);
  else next.add(f.id);
  return next;
}

/** Shift-click: set every selectable row between `anchorId` and `id` to `value`. */
export function selectRange(
  sel: Selection,
  ordered: readonly Finding[],
  anchorId: string | null,
  id: string,
  value: boolean,
): Set<string> {
  const next = new Set(sel);
  const a = anchorId ? ordered.findIndex((f) => f.id === anchorId) : -1;
  const b = ordered.findIndex((f) => f.id === id);
  if (b < 0) return next;
  const [lo, hi] = a < 0 ? [b, b] : [Math.min(a, b), Math.max(a, b)];
  for (const f of ordered.slice(lo, hi + 1)) {
    if (!isSelectable(f)) continue;
    if (value) next.add(f.id);
    else next.delete(f.id);
  }
  return next;
}

export function selectAll(items: readonly Finding[]): Set<string> {
  return new Set(items.filter(isSelectable).map((f) => f.id));
}

export function selectAllSafe(sel: Selection, items: readonly Finding[]): Set<string> {
  const next = new Set(sel);
  for (const f of items) if (isSelectable(f) && f.tier.kind === "safe") next.add(f.id);
  return next;
}

export function defaultSelection(items: readonly Finding[]): Set<string> {
  return selectAllSafe(new Set(), items);
}

export type GroupState = "all" | "some" | "none";

export function groupState(sel: Selection, group: readonly Finding[]): GroupState {
  const selectable = group.filter(isSelectable);
  if (selectable.length === 0) return "none";
  const n = selectable.filter((f) => sel.has(f.id)).length;
  return n === 0 ? "none" : n === selectable.length ? "all" : "some";
}

/** Tri-state group checkbox: all → none; some/none → all. */
export function toggleGroup(sel: Selection, group: readonly Finding[]): Set<string> {
  const next = new Set(sel);
  const all = groupState(sel, group) === "all";
  for (const f of group.filter(isSelectable)) {
    if (all) next.delete(f.id);
    else next.add(f.id);
  }
  return next;
}

export interface SelectionTotals {
  count: number;
  bytes: number;
  /** Removed right away (delete, commands, emptying the bin). */
  freesNow: number;
  /** Moved to the Recycle Bin. */
  toRecycleBin: number;
}

export function totals(sel: Selection, items: readonly Finding[]): SelectionTotals {
  let count = 0;
  let freesNow = 0;
  let toRecycleBin = 0;
  for (const f of items) {
    if (!sel.has(f.id) || !isSelectable(f)) continue;
    count += 1;
    if (f.disposal.kind === "recycle") toRecycleBin += f.bytes;
    else freesNow += f.bytes;
  }
  return { count, bytes: freesNow + toRecycleBin, freesNow, toRecycleBin };
}

export type SortKey = "size" | "age" | "name" | "tier";

const tierRank = (f: Finding) => (f.tier.kind === "safe" ? 0 : f.tier.kind === "review" ? 1 : 2);

export function sortFindings(items: readonly Finding[], key: SortKey): Finding[] {
  const v = [...items];
  switch (key) {
    case "size":
      return v.sort((a, b) => b.bytes - a.bytes);
    case "age":
      return v.sort((a, b) => (a.dates.modifiedMax ?? 0) - (b.dates.modifiedMax ?? 0));
    case "name":
      return v.sort((a, b) => a.title.localeCompare(b.title));
    case "tier":
      return v.sort((a, b) => tierRank(a) - tierRank(b) || b.bytes - a.bytes);
  }
}

export interface Group {
  source: string;
  sourcePath: string | null;
  items: Finding[];
  bytes: number;
}

/** Group by source (tool or folder), groups ordered by total size. */
export function groupBySource(items: readonly Finding[]): Group[] {
  const map = new Map<string, Group>();
  for (const f of items) {
    const g = map.get(f.source) ?? { source: f.source, sourcePath: f.sourcePath, items: [], bytes: 0 };
    g.items.push(f);
    if (f.tier.kind !== "protected") g.bytes += f.bytes;
    map.set(f.source, g);
  }
  return [...map.values()].sort((a, b) => b.bytes - a.bytes);
}
