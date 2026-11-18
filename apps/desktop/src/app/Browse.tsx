import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, File, Folder, FolderOpen, HardDrive, Link2, Lock, ShieldCheck, Trash2, Zap } from "lucide-react";
import { Badge, Button, Chip, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { ago, formatBytes, protectExplain } from "../lib/format";
import { useT } from "../i18n";
import { squarify } from "../lib/treemap";
import type { BrowseChild, BrowseView, FileKind, Finding } from "../lib/types";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { useScan } from "../state/scan";
import { Sheet } from "../components/Sheet";

const KIND_HUE: Record<FileKind, string> = {
  images: "blue",
  videos: "pink",
  audio: "violet",
  documents: "teal",
  archives: "amber",
  installers: "orange",
  code: "green",
  "ai-models": "violet",
  other: "slate",
};

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function fill(c: BrowseChild): { bg: string; fg: string } {
  if (c.isDir) {
    if (c.leaf !== "none") return { bg: cssVar("--pare-cat-slate-bg"), fg: cssVar("--pare-cat-slate-fg") };
    return { bg: cssVar("--pare-accent-subtle"), fg: cssVar("--pare-accent-on-subtle") };
  }
  const hue = KIND_HUE[c.kind ?? "other"];
  return { bg: cssVar(`--pare-cat-${hue}-bg`), fg: cssVar(`--pare-cat-${hue}-fg`) };
}

interface Cell extends BrowseChild {
  value: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

function Treemap({ items, onOpen, onPick }: { items: BrowseChild[]; onOpen: (c: BrowseChild) => void; onPick: (c: BrowseChild) => void }) {
  const { t, tp, locale } = useT();
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [hover, setHover] = useState<Cell | null>(null);
  const theme = useApp((s) => s.settings?.theme);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const cells: Cell[] = useMemo(
    () => squarify(items.map((c) => ({ ...c, value: c.bytes })), { x: 0, y: 0, w: size.w, h: size.h }).filter((c) => c.w > 0 && c.h > 0),
    [items, size],
  );

  useEffect(() => {
    const cv = canvas.current;
    if (!cv || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = size.w * dpr;
    cv.height = size.h * dpr;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, size.w, size.h);
    const border = cssVar("--pare-bg-surface");
    const font = cssVar("--pare-font-sans");
    for (const c of cells) {
      const { bg, fg } = fill(c);
      ctx.fillStyle = bg;
      ctx.fillRect(c.x, c.y, c.w, c.h);
      ctx.strokeStyle = border;
      ctx.lineWidth = 2;
      ctx.strokeRect(c.x + 1, c.y + 1, c.w - 2, c.h - 2);
      if (c.w > 70 && c.h > 34) {
        ctx.fillStyle = fg;
        ctx.font = `600 12px ${font}`;
        const name = c.name.length > Math.floor(c.w / 7) ? `${c.name.slice(0, Math.max(3, Math.floor(c.w / 7) - 1))}…` : c.name;
        ctx.fillText(name, c.x + 8, c.y + 18);
        ctx.font = `400 11px ${font}`;
        ctx.fillText(formatBytes(c.bytes), c.x + 8, c.y + 33);
      }
    }
    if (hover) {
      ctx.strokeStyle = cssVar("--pare-accent-default");
      ctx.lineWidth = 2;
      ctx.strokeRect(hover.x + 1, hover.y + 1, hover.w - 2, hover.h - 2);
    }
  }, [cells, size, hover, theme, locale]);

  const at = (e: React.MouseEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    return cells.find((c) => x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h) ?? null;
  };

  return (
    <div ref={wrap} className="relative h-full min-h-[280px] w-full overflow-hidden rounded-lg border border-border-default bg-bg-surface">
      <canvas
        ref={canvas}
        role="img"
        aria-label={t("browse.mapLabel", { list: items.slice(0, 5).map((c) => `${c.name} ${formatBytes(c.bytes)}`).join(", ") })}
        style={{ width: size.w, height: size.h }}
        onMouseMove={(e) => setHover(at(e))}
        onMouseLeave={() => setHover(null)}
        onClick={(e) => {
          const c = at(e);
          if (!c) return;
          if (c.isDir && c.leaf === "none") onOpen(c);
          else onPick(c);
        }}
        className="cursor-pointer"
      />
      {hover && (
        <div className="pointer-events-none absolute left-3 top-3 rounded-md bg-bg-inverse px-3 py-1.5 text-caption text-on-inverse-text shadow-floating">
          {hover.name} · {formatBytes(hover.bytes)}
          {hover.isDir ? ` · ${tp("count.files", hover.files)}` : ""}
        </div>
      )}
    </div>
  );
}

export function Browse() {
  const { t, tp } = useT();
  const info = useApp((s) => s.info);
  const go = useApp((s) => s.go);
  const scan = useScan();
  const run = useClean((s) => s.run);
  const [view, setView] = useState<BrowseView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<FileKind | null>(null);
  const [kindFiles, setKindFiles] = useState<BrowseChild[]>([]);
  const [picked, setPicked] = useState<{ child: BrowseChild; finding: Finding | null } | null>(null);

  const open = useCallback(async (path?: string) => {
    try {
      setView(await api.browseView(path));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void open();
  }, [open, scan.summary?.finished]);

  useEffect(() => {
    if (kind) void api.browseKindFiles(kind).then(setKindFiles);
  }, [kind]);

  async function pick(child: BrowseChild) {
    setPicked({ child, finding: null });
    if (child.leaf !== "none") return;
    try {
      const f = await api.browseItem(child.path);
      setPicked({ child, finding: f });
    } catch (e) {
      setError(String(e));
    }
  }

  const crumbs = useMemo(() => {
    if (!view) return [];
    const root = view.roots.find((r) => view.path.toLowerCase().startsWith(r.path.toLowerCase()));
    if (!root) return [];
    const rest = view.path.slice(root.path.length).split(/[\\/]/).filter(Boolean);
    const out = [{ name: root.name, path: root.path }];
    let acc = root.path.replace(/[\\/]$/, "");
    for (const part of rest) {
      acc = `${acc}\\${part}`;
      out.push({ name: part, path: acc });
    }
    return out;
  }, [view]);

  const foldedName = tp("browse.smallerFiles", view?.foldedFiles ?? 0);
  const items: BrowseChild[] = useMemo(() => {
    if (!view) return [];
    const list = [...view.children];
    if (view.foldedBytes > 0)
      list.push({ name: foldedName, path: view.path, bytes: view.foldedBytes, files: view.foldedFiles, isDir: false, kind: "other", leaf: "excluded", modified: 0 });
    return list;
  }, [view, foldedName]);

  const fastNote = view && view.fastDrives.length > 0;
  const showElevate = info && !info.elevated && info.platform === "windows";

  if (!view) {
    return (
      <div className="flex min-h-full flex-col items-center justify-center gap-4 px-8 text-center">
        <span className="flex size-14 items-center justify-center rounded-xl bg-bg-subtle text-text-secondary">
          <HardDrive size={26} aria-hidden="true" />
        </span>
        <h1 className="text-heading-h2">{t("browse.empty.title")}</h1>
        <p className="max-w-[460px] text-body-m text-text-secondary">{error ? t("browse.empty.body") : t("common.loading")}</p>
        <Button
          icon={HardDrive}
          onClick={() => {
            useScan.setState({ scope: ["pc"] });
            void scan.start();
          }}
        >
          {t("common.scanThisPc")}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 pb-6 pl-4 pr-8 pt-6">
      <header className="flex flex-wrap items-center gap-3">
        <div className="mr-auto min-w-0">
          <h1 className="text-heading-h1">{t("browse.title")}</h1>
          <p className="text-body-s text-text-secondary">
            {t("browse.summary", { size: formatBytes(view.bytes), files: tp("count.files", view.files), ago: ago(view.scanned) })}
          </p>
        </div>
        {fastNote ? (
          <Badge tone="accent">
            <Zap size={12} aria-hidden="true" /> {t("browse.fastMode")}
          </Badge>
        ) : showElevate ? (
          <Button variant="ghost" icon={Zap} onClick={() => void api.relaunchElevated().catch((e) => setError(String(e)))}>
            {t("browse.elevate")}
          </Button>
        ) : null}
        {view.roots.length > 1 && (
          <div className="flex gap-2" role="group" aria-label={t("browse.drives")}>
            {view.roots.map((r) => (
              <Chip key={r.path} icon={HardDrive} selected={view.path.toLowerCase().startsWith(r.path.toLowerCase())} onClick={() => void open(r.path)}>
                {r.name}
              </Chip>
            ))}
          </div>
        )}
      </header>

      {view.kinds.length > 0 && (
        <div className="flex flex-wrap gap-2" role="group" aria-label={t("browse.fileTypes")}>
          {view.kinds
            .slice()
            .sort((a, b) => b.bytes - a.bytes)
            .map((k) => (
              <Chip key={k.kind} selected={kind === k.kind} onClick={() => setKind(kind === k.kind ? null : k.kind)}>
                {k.label} · {formatBytes(k.bytes)}
              </Chip>
            ))}
        </div>
      )}

      <nav aria-label={t("browse.path")} className="flex flex-wrap items-center gap-1 text-body-s">
        {crumbs.map((c, i) => (
          <span key={c.path} className="flex items-center gap-1">
            {i > 0 && <ChevronRight size={14} className="text-text-tertiary" aria-hidden="true" />}
            <button type="button" onClick={() => void open(c.path)} className={cx("rounded px-1.5 py-0.5 hover:bg-bg-subtle", focusRing, i === crumbs.length - 1 ? "text-text-primary" : "text-text-secondary")}>
              {c.name}
            </button>
          </span>
        ))}
      </nav>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 min-[1100px]:grid-cols-[minmax(0,1fr)_340px]">
        <Treemap items={kind ? kindFiles : items} onOpen={(c) => void open(c.path)} onPick={(c) => void pick(c)} />
        <div className="min-h-0 overflow-y-auto rounded-lg border border-border-default bg-bg-surface">
          <ul aria-label={kind ? t("browse.largestOfType") : t("browse.contents")}>
            {(kind ? kindFiles : items).map((c) => {
              const Icon = c.leaf === "link" ? Link2 : c.leaf === "excluded" || c.leaf === "error" ? Lock : c.isDir ? Folder : File;
              const pct = view.bytes ? (c.bytes / view.bytes) * 100 : 0;
              return (
                <li key={c.path + c.name} className="border-b border-border-default last:border-b-0">
                  <button
                    type="button"
                    onClick={() => (c.isDir && c.leaf === "none" ? void open(c.path) : void pick(c))}
                    className={cx("flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-bg-subtle", focusRing)}
                    title={c.path}
                  >
                    <Icon size={16} className="shrink-0 text-text-secondary" aria-hidden="true" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-body-m">{c.name}</span>
                      <span className="mt-1 block h-1 overflow-hidden rounded-full bg-bg-subtle">
                        <span className="block h-full rounded-full bg-accent-default" style={{ width: `${Math.max(1, Math.min(100, pct))}%` }} />
                      </span>
                    </span>
                    <span className="tabular shrink-0 text-body-s text-text-secondary">{formatBytes(c.bytes)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          {view.foldedBytes > 0 && !kind && (
            <p className="px-3 py-2 text-caption text-text-tertiary">{t("browse.folded")}</p>
          )}
        </div>
      </div>

      <Sheet
        open={picked !== null}
        onClose={() => setPicked(null)}
        placement="side"
        title={picked?.child.name ?? ""}
        description={picked ? `${formatBytes(picked.child.bytes)} · ${picked.child.path}` : undefined}
        footer={
          picked && (
            <>
              <Button variant="secondary" icon={FolderOpen} onClick={() => void api.openInExplorer(picked.child.path)}>
                {t("common.openInExplorer")}
              </Button>
              {picked.finding && picked.finding.tier.kind !== "protected" && (
                <Button
                  icon={Trash2}
                  onClick={() => {
                    const id = picked.finding!.id;
                    setPicked(null);
                    void run([id]);
                  }}
                >
                  {t("common.moveToRecycleBin")}
                </Button>
              )}
            </>
          )
        }
      >
        {picked && picked.child.leaf !== "none" && (
          <p className="flex items-start gap-2 text-body-s text-text-secondary">
            <ShieldCheck size={16} className="mt-0.5 text-safe-fg" aria-hidden="true" />
            {picked.child.leaf === "link"
              ? t("browse.leaf.link")
              : picked.child.leaf === "excluded"
                ? t("browse.leaf.excluded")
                : t("browse.leaf.error")}
          </p>
        )}
        {picked?.finding?.tier.kind === "protected" && (
          <p className="text-body-s text-keep-fg">
            {t("common.kept", { reason: picked.finding.tier.detail ?? protectExplain(picked.finding.tier.reason) })}
          </p>
        )}
        {picked?.finding && picked.finding.tier.kind !== "protected" && (
          <p className="text-body-s text-text-secondary">{t("browse.recycleNote")}</p>
        )}
        <div className="mt-4">
          <Button variant="ghost" onClick={() => go({ name: "clean" })}>
            {t("browse.seeSuggestions")}
          </Button>
        </div>
      </Sheet>
      {error && <p className="text-body-s text-keep-fg">{error}</p>}
    </div>
  );
}
