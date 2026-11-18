import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowUpDown, Check, ChevronLeft, EyeOff } from "lucide-react";
import { Button, Checkbox, FileRow, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { formatBytes, plural, tierBadge } from "../lib/format";
import {
  defaultSelection,
  groupBySource,
  groupState,
  isSelectable,
  selectAll,
  selectAllSafe,
  selectRange,
  sortFindings,
  toggle,
  toggleGroup,
  totals,
  type SortKey,
} from "../lib/selection";
import type { CategoryDetail, CategoryId, Finding } from "../lib/types";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { categoryHue, findingIcon } from "../components/icons";
import { Banner } from "../components/Banner";
import { Sheet } from "../components/Sheet";
import { DetailsPanel } from "./DetailsPanel";

const BANNER: Partial<Record<CategoryId, string>> = {
  "ai-tools":
    "Sessions used in the last 24 hours and files an app has open are never suggested. Your own files go to the Recycle Bin; caches that rebuild are removed directly.",
  "dev-caches":
    "Package caches re-download when a project needs them. Tools that are running keep their caches until they close.",
  downloads:
    "Installers and old downloads go to the Recycle Bin, so you can restore them. Unfinished downloads are removed directly.",
  "large-files":
    "Large files go to the Recycle Bin. Anything changed in the last 24 hours or open in an app is never suggested.",
  "recycle-bin": "Emptying the Recycle Bin removes these files for good. Pare only does it when you ask.",
  "empty-folders": "Only folders with no files inside are listed. Removing them tidies up but frees almost no space.",
  junk: "Temporary files older than a day are removed directly; files an app still has open are skipped.",
};
const DEFAULT_BANNER =
  "Anything changed in the last 24 hours or open in an app is never suggested. Your own files go to the Recycle Bin; caches that rebuild are removed directly.";

const SORTS: { key: SortKey; label: string }[] = [
  { key: "size", label: "Size" },
  { key: "age", label: "Age" },
  { key: "name", label: "Name" },
  { key: "tier", label: "Tier" },
];

type Row = { kind: "group"; source: string; path: string | null; items: Finding[]; bytes: number } | { kind: "item"; f: Finding };

function useWide(): boolean {
  const q = "(min-width: 1100px)";
  const [wide, setWide] = useState(() => window.matchMedia(q).matches);
  useEffect(() => {
    const m = window.matchMedia(q);
    const on = () => setWide(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return wide;
}

export function Review({ category }: { category: CategoryId }) {
  const go = useApp((s) => s.go);
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const runClean = useClean((s) => s.run);
  const cleaning = useClean((s) => s.running);
  const [detail, setDetail] = useState<CategoryDetail | null>(null);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<string | null>(null);
  const [sort, setSort] = useState<SortKey>("size");
  const [sortOpen, setSortOpen] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);
  const [focusIdx, setFocusIdx] = useState(0);
  const [confirm, setConfirm] = useState<string[] | null>(null);
  const shift = useRef(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const wide = useWide();

  const load = useCallback(async () => {
    const d = await api.categoryGet(category);
    setDetail(d);
    return d;
  }, [category]);

  useEffect(() => {
    void load().then((d) => setSel(defaultSelection(d.findings)));
  }, [load]);

  const findings = useMemo(() => detail?.findings ?? [], [detail]);
  const groups = useMemo(() => groupBySource(sortFindings(findings, sort)), [findings, sort]);
  const rows: Row[] = useMemo(
    () =>
      groups.flatMap((g) => [
        { kind: "group" as const, source: g.source, path: g.sourcePath, items: g.items, bytes: g.bytes },
        ...g.items.map((f) => ({ kind: "item" as const, f })),
      ]),
    [groups],
  );
  const ordered = useMemo(() => groups.flatMap((g) => g.items), [groups]);
  const t = totals(sel, findings);
  const open = findings.find((f) => f.id === openId) ?? null;
  const summary = detail?.summary;
  const totalBytes = (summary?.safeBytes ?? 0) + (summary?.reviewBytes ?? 0);

  const virt = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (rows[i]?.kind === "group" ? 44 : 59),
    overscan: 12,
  });

  const itemRowIndexes = useMemo(() => rows.flatMap((r, i) => (r.kind === "item" ? [i] : [])), [rows]);

  function onRowCheck(f: Finding, checked: boolean) {
    if (shift.current && anchor) setSel((s) => selectRange(s, ordered, anchor, f.id, checked));
    else setSel((s) => toggle(s, f));
    setAnchor(f.id);
  }

  function onKey(e: KeyboardEvent<HTMLDivElement>) {
    const pos = itemRowIndexes.indexOf(focusIdx);
    const focusRow = (i: number) => {
      setFocusIdx(i);
      virt.scrollToIndex(i);
      requestAnimationFrame(() => scrollRef.current?.querySelector<HTMLElement>(`[data-row="${i}"]`)?.focus());
    };
    if (e.key === "ArrowDown" && pos < itemRowIndexes.length - 1) {
      e.preventDefault();
      focusRow(itemRowIndexes[pos + 1]!);
    } else if (e.key === "ArrowUp" && pos > 0) {
      e.preventDefault();
      focusRow(itemRowIndexes[pos - 1]!);
    } else if ((e.key === "a" || e.key === "A") && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      setSel(selectAll(findings));
    } else if (e.key === " ") {
      const r = rows[focusIdx];
      if (r?.kind === "item") {
        e.preventDefault();
        if (e.shiftKey && anchor) setSel((s) => selectRange(s, ordered, anchor, r.f.id, !s.has(r.f.id)));
        else setSel((s) => toggle(s, r.f));
        setAnchor(r.f.id);
      }
    } else if (e.key === "Enter") {
      const r = rows[focusIdx];
      if (r?.kind === "item") setOpenId(r.f.id);
    } else if (e.key === "Escape") {
      setOpenId(null);
    }
  }

  function freeUp(ids: string[]) {
    const chosen = findings.filter((f) => ids.includes(f.id) && isSelectable(f));
    if (chosen.some((f) => f.tier.kind === "review")) setConfirm(chosen.map((f) => f.id));
    else void runClean(chosen.map((f) => f.id));
  }

  const confirmTotals = confirm ? totals(new Set(confirm), findings) : null;

  const panel = open && (
    <DetailsPanel
      finding={open}
      onClose={() => setOpenId(null)}
      onDispose={() => freeUp([open.id])}
      busy={cleaning}
      onKeep={async (path) => {
        const s = await api.exclusionAdd(path);
        useApp.getState().setSettings(s);
        await load();
      }}
    />
  );

  return (
    <div className="flex h-full min-h-0">
      <div className="relative flex min-w-0 flex-1 flex-col gap-4 pb-0 pl-4 pr-8 pt-6">
        <header className="flex items-center gap-3">
          <button
            type="button"
            aria-label="Back to Clean"
            onClick={() => go({ name: "clean" })}
            className={cx("flex size-9 items-center justify-center rounded-md border border-border-default bg-bg-surface text-text-secondary hover:bg-bg-subtle", focusRing)}
          >
            <ChevronLeft size={18} aria-hidden="true" />
          </button>
          <div className="mr-auto min-w-0">
            <h1 className="truncate text-heading-h2">{summary?.title ?? "Review"}</h1>
            <p className="tabular text-body-s text-text-secondary">
              {formatBytes(totalBytes)} · {plural(summary?.items ?? 0, "item")}
              {summary && summary.sources > 1 ? ` · ${plural(summary.sources, "source")}` : ""}
            </p>
          </div>
          <Button variant="ghost" icon={Check} onClick={() => setSel((s) => selectAllSafe(s, findings))}>
            Select all safe
          </Button>
          <div className="relative">
            <Button variant="secondary" icon={ArrowUpDown} aria-haspopup="menu" aria-expanded={sortOpen} onClick={() => setSortOpen((o) => !o)}>
              Sort: {SORTS.find((s) => s.key === sort)?.label}
            </Button>
            {sortOpen && (
              <ul role="menu" className="absolute right-0 z-20 mt-1 w-40 overflow-hidden rounded-md border border-border-default bg-bg-surface py-1 shadow-floating">
                {SORTS.map((s) => (
                  <li key={s.key}>
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={s.key === sort}
                      onClick={() => {
                        setSort(s.key);
                        setSortOpen(false);
                      }}
                      className={cx("flex w-full items-center justify-between px-3 py-2 text-left text-body-m hover:bg-bg-subtle", focusRing)}
                    >
                      {s.label}
                      {s.key === sort && <Check size={16} className="text-accent-default" aria-hidden="true" />}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </header>

        <Banner>{BANNER[category] ?? DEFAULT_BANNER}</Banner>

        <div
          ref={scrollRef}
          role="grid"
          aria-label={`${summary?.title ?? "Items"}, ${plural(findings.length, "item")}`}
          aria-multiselectable="true"
          onKeyDown={onKey}
          onPointerDownCapture={(e) => (shift.current = e.shiftKey)}
          className="min-h-0 flex-1 overflow-y-auto rounded-t-lg border border-b-0 border-border-default bg-bg-surface pb-24"
        >
          {!detail ? (
            <p className="p-6 text-body-m text-text-secondary">Loading…</p>
          ) : findings.length === 0 ? (
            <p className="p-6 text-body-m text-text-secondary">Nothing left here.</p>
          ) : (
            <div style={{ height: virt.getTotalSize(), position: "relative" }}>
              {virt.getVirtualItems().map((v) => {
                const r = rows[v.index]!;
                return (
                  <div
                    key={v.key}
                    data-index={v.index}
                    ref={virt.measureElement}
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${v.start}px)` }}
                  >
                    {r.kind === "group" ? (
                      <GroupHeader
                        source={r.source}
                        path={r.path}
                        bytes={r.bytes}
                        state={groupState(sel, r.items)}
                        disabled={!r.items.some(isSelectable)}
                        onToggle={() => setSel((s) => toggleGroup(s, r.items))}
                      />
                    ) : (
                      <div
                        role="row"
                        data-row={v.index}
                        tabIndex={v.index === focusIdx || (focusIdx === 0 && v.index === 1) ? 0 : -1}
                        aria-selected={sel.has(r.f.id)}
                        onFocus={() => setFocusIdx(v.index)}
                        className={cx("outline-none focus-visible:relative focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-default", openId === r.f.id && "[&>div]:bg-accent-subtle")}
                      >
                        <FileRow
                          icon={findingIcon(r.f)}
                          hue={categoryHue(r.f.category)}
                          name={r.f.title}
                          path={r.f.paths[0] ?? ""}
                          badge={(() => {
                            const b = tierBadge(r.f.tier, r.f.badge, r.f.disposal);
                            return { tone: b.tone, label: b.label };
                          })()}
                          age={r.f.tier.kind === "protected" ? (r.f.tier.detail ?? r.f.note ?? undefined) : (r.f.note ?? undefined)}
                          size={r.f.bytes}
                          checked={sel.has(r.f.id)}
                          disabled={!isSelectable(r.f)}
                          onCheckedChange={(c) => onRowCheck(r.f, c)}
                          onClick={(e) => {
                            if ((e.target as HTMLElement).closest("input,label")) return;
                            setOpenId(r.f.id);
                          }}
                          className="cursor-pointer hover:[&>div]:bg-bg-subtle"
                        />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
          {detail && findings.length > 0 && (
            <div className="px-4 py-6">
              <Button
                variant="ghost"
                icon={EyeOff}
                onClick={async () => {
                  await updateSettings({ hiddenCards: [...(settings?.hiddenCards ?? []), category] });
                  go({ name: "clean" });
                }}
              >
                Hide this suggestion
              </Button>
            </div>
          )}
        </div>

        {t.count > 0 && (
          <div
            role="region"
            aria-label="Selection"
            className="pare-rise absolute bottom-6 left-1/2 z-10 flex w-max max-w-[calc(100%-48px)] -translate-x-1/2 items-center gap-6 whitespace-nowrap rounded-lg bg-bg-inverse py-2.5 pl-5 pr-2.5 shadow-floating"
          >
            <div className="tabular">
              <p className="text-body-m-strong text-on-inverse-text">
                {plural(t.count, "selected", "selected")} · {formatBytes(t.bytes)}
              </p>
              <p className="text-caption text-on-inverse-text-muted">
                {formatBytes(t.freesNow)} removed now · {formatBytes(t.toRecycleBin)} to Recycle Bin
              </p>
            </div>
            <button
              type="button"
              onClick={() => setSel(new Set())}
              className={cx("rounded-md px-3 py-2 text-label-m text-on-inverse-text-muted hover:text-on-inverse-text", focusRing)}
            >
              Clear
            </button>
            <Button onClick={() => freeUp([...sel])} disabled={cleaning}>
              {cleaning ? "Freeing up…" : `Free up ${formatBytes(t.bytes)}`}
            </Button>
          </div>
        )}
      </div>

      {open && wide && <aside className="w-[320px] shrink-0 overflow-y-auto border-l border-border-default bg-bg-surface">{panel}</aside>}
      {open && !wide && (
        <Sheet open placement="side" onClose={() => setOpenId(null)} title="Details" className="[&_h2#sheet-title]:sr-only">
          {panel}
        </Sheet>
      )}

      <Sheet
        open={confirm !== null}
        onClose={() => setConfirm(null)}
        title={`Free up ${formatBytes(confirmTotals?.bytes ?? 0)}?`}
        description="Some of these are your own files. They go to the Recycle Bin, so you can restore them from History."
        footer={
          <>
            <Button variant="secondary" onClick={() => setConfirm(null)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                const ids = confirm ?? [];
                setConfirm(null);
                void runClean(ids);
              }}
            >
              Free up {formatBytes(confirmTotals?.bytes ?? 0)}
            </Button>
          </>
        }
      >
        {confirmTotals && (
          <ul className="divide-y divide-border-default text-body-m">
            <li className="flex justify-between py-2.5">
              <span>Removed now (rebuildable)</span>
              <span className="tabular text-text-secondary">{formatBytes(confirmTotals.freesNow)}</span>
            </li>
            <li className="flex justify-between py-2.5">
              <span>Moved to the Recycle Bin</span>
              <span className="tabular text-text-secondary">{formatBytes(confirmTotals.toRecycleBin)}</span>
            </li>
          </ul>
        )}
      </Sheet>
    </div>
  );
}

function GroupHeader(props: {
  source: string;
  path: string | null;
  bytes: number;
  state: "all" | "some" | "none";
  disabled: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="flex items-center gap-3 border-b border-border-default bg-bg-subtle px-4 py-2.5">
      <Checkbox
        aria-label={`Select all in ${props.source}`}
        checked={props.state === "all"}
        indeterminate={props.state === "some"}
        disabled={props.disabled}
        onChange={props.onToggle}
      />
      <span className="text-body-m-strong text-text-primary">{props.source}</span>
      {props.path && <span className="min-w-0 truncate font-mono text-mono-s text-text-tertiary">{props.path}</span>}
      <span className="tabular ml-auto text-body-m-strong text-text-primary">{formatBytes(props.bytes)}</span>
    </div>
  );
}
