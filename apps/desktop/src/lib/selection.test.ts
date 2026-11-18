import { describe, expect, it } from "vitest";
import {
  defaultSelection,
  groupBySource,
  groupState,
  selectAll,
  selectAllSafe,
  selectRange,
  sortFindings,
  toggle,
  toggleGroup,
  totals,
} from "./selection";
import type { Finding, Tier, Disposal } from "./types";

function f(id: string, tier: Tier, bytes = 100, disposal: Disposal = { kind: "delete" }, source = "A"): Finding {
  return {
    id, ruleId: "r", category: "ai-tools", source, sourcePath: null, title: id, paths: [`C:\\${id}`],
    bytes, logicalBytes: bytes, items: 1, tier, disposal, comesBack: "automatically", why: "", ifRemoved: "",
    dates: { createdMin: null, modifiedMax: bytes, accessedMax: null }, createdBy: null, badge: null,
    note: null, git: null, facts: [],
  };
}
const SAFE: Tier = { kind: "safe" };
const REVIEW: Tier = { kind: "review" };
const KEEP: Tier = { kind: "protected", reason: "in-use" };

const items = [
  f("a", SAFE, 300),
  f("b", REVIEW, 200, { kind: "recycle" }),
  f("c", KEEP, 900),
  f("d", SAFE, 50, { kind: "recycle" }, "B"),
  f("e", REVIEW, 10, { kind: "guide", steps: [] }, "B"),
];

describe("selection", () => {
  it("never selects protected or guide-only rows", () => {
    expect(toggle(new Set(), items[2]!).has("c")).toBe(false);
    expect([...selectAll(items)].sort()).toEqual(["a", "b", "d"]);
    expect(selectRange(new Set(), items, "a", "e", true).has("c")).toBe(false);
    expect(selectRange(new Set(), items, "a", "e", true).has("e")).toBe(false);
    expect(toggleGroup(new Set(), items.filter((x) => x.source === "A")).has("c")).toBe(false);
  });

  it("defaults to Safe items and 'Select all safe' adds only Safe", () => {
    expect([...defaultSelection(items)].sort()).toEqual(["a", "d"]);
    expect([...selectAllSafe(new Set(["b"]), items)].sort()).toEqual(["a", "b", "d"]);
  });

  it("shift-click ranges work in both directions and can clear", () => {
    expect([...selectRange(new Set(), items, "d", "a", true)].sort()).toEqual(["a", "b", "d"]);
    expect([...selectRange(new Set(["a", "b", "d"]), items, "a", "b", false)]).toEqual(["d"]);
    expect([...selectRange(new Set(), items, null, "b", true)]).toEqual(["b"]);
  });

  it("tri-state groups ignore protected rows", () => {
    const groupA = items.filter((x) => x.source === "A");
    expect(groupState(new Set(), groupA)).toBe("none");
    expect(groupState(new Set(["a"]), groupA)).toBe("some");
    expect(groupState(new Set(["a", "b"]), groupA)).toBe("all");
    expect(toggleGroup(new Set(["a", "b"]), groupA).size).toBe(0);
    expect([...toggleGroup(new Set(["a"]), groupA)].sort()).toEqual(["a", "b"]);
  });

  it("splits totals into removed now vs Recycle Bin", () => {
    const t = totals(new Set(["a", "b", "c", "d"]), items);
    expect(t).toEqual({ count: 3, bytes: 550, freesNow: 300, toRecycleBin: 250 });
  });

  it("groups by source ordered by size; sorts by key", () => {
    expect(groupBySource(items).map((g) => g.source)).toEqual(["A", "B"]);
    expect(groupBySource(items)[0]!.bytes).toBe(500);
    expect(sortFindings(items, "size")[0]!.id).toBe("c");
    expect(sortFindings(items, "tier").map((x) => x.id)).toEqual(["a", "d", "b", "e", "c"]);
    expect(sortFindings(items, "name")[0]!.id).toBe("a");
  });
});
