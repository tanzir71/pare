import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, File, Folder, FolderOpen, HardDrive, Link2, Lock, ShieldCheck, Trash2, Zap } from "lucide-react";
import { Badge, Button, Chip, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { ago, formatBytes, formatCount, protectExplain } from "../lib/format";
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
  }, [cells, size, hover, theme]);

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
        aria-label={`Storage map: ${items.slice(0, 5).map((c) => `${c.name} ${formatBytes(c.bytes)}`).join(", ")}`}
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
          {hover.isDir ? ` · ${formatCount(hover.files)} files` : ""}
        </div>
      )}
    </div>
  );
}

export function Browse() {
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

  const items: BrowseChild[] = useMemo(() => {
    if (!view) return [];
    const list = [...view.children];
    if (view.foldedBytes > 0)
      list.push({ name: `${formatCount(view.foldedFiles)} smaller files`, path: view.path, bytes: view.foldedBytes, files: view.foldedFiles, isDir: false, kind: "other", leaf: "excluded", modified: 0 });
    return list;
  }, [view]);

  const fastNote = view && view.fastDrives.length > 0;
  const showElevate = info && !info.elevated && info.platform === "windows";

  if (!view) {
    return (
      <div className="flex min-h-full flex-col items-center justify-center gap-4 px-8 text-center">
        <span className="flex size-14 items-center justify-center rounded-xl bg-bg-subtle text-text-secondary">
          <HardDrive size={26} aria-hidden="true" />
        </span>
        <h1 className="text-heading-h2">See where your space goes</h1>
        <p className="max-w-[460px] text-body-m text-text-secondary">
          {error ? "Browse shows the storage map of your last scan. Scan this PC to build it." : "Loading…"}
        </p>
        <Button
          icon={HardDrive}
          onClick={() => {
            useScan.setState({ scope: ["pc"] });
            void scan.start();
          }}
        >
          Scan this PC
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 pb-6 pl-4 pr-8 pt-6">
      <header className="flex flex-wrap items-center gap-3">
        <div className="mr-auto min-w-0">
          <h1 className="text-heading-h1">Browse</h1>
          <p className="text-body-s text-text-secondary">
            {formatBytes(view.bytes)} in {formatCount(view.files)} files · scanned {ago(view.scanned)}
          </p>
        </div>
        {fastNote ? (
          <Badge tone="accent">
            <Zap size={12} aria-hidden="true" /> Fast mode
          </Badge>
        ) : showElevate ? (
          <Button variant="ghost" icon={Zap} onClick={() => void api.relaunchElevated().catch((e) => setError(String(e)))}>
            Restart as administrator for fast mode
          </Button>
        ) : null}
        {view.roots.length > 1 && (
          <div className="flex gap-2" role="group" aria-label="Drives">
            {view.roots.map((r) => (
              <Chip key={r.path} icon={HardDrive} selected={view.path.toLowerCase().startsWith(r.path.toLowerCase())} onClick={() => void open(r.path)}>
                {r.name}
              </Chip>
            ))}
          </div>
        )}
      </header>

      {view.kinds.length > 0 && (
        <div className="flex flex-wrap gap-2" role="group" aria-label="File types">
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

      <nav aria-label="Folder path" className="flex flex-wrap items-center gap-1 text-body-s">
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
          <ul aria-label={kind ? "Largest files of this type" : "Contents"}>
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
            <p className="px-3 py-2 text-caption text-text-tertiary">Files under 10 MB are counted, not listed, in whole-drive scans.</p>
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
                Open in Explorer
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
                  Move to Recycle Bin
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
              ? "A link to another folder. Pare never follows or removes links from Browse."
              : picked.child.leaf === "excluded"
                ? "A system or protected location. Pare shows its size but never touches it."
                : "Pare couldn't read this folder."}
          </p>
        )}
        {picked?.finding?.tier.kind === "protected" && (
          <p className="text-body-s text-keep-fg">Kept: {picked.finding.tier.detail ?? protectExplain(picked.finding.tier.reason)}.</p>
        )}
        {picked?.finding && picked.finding.tier.kind !== "protected" && (
          <p className="text-body-s text-text-secondary">
            It goes to the Recycle Bin, so you can restore it from History. Pare checked it isn't in use, recently changed or a protected location.
          </p>
        )}
        <div className="mt-4">
          <Button variant="ghost" onClick={() => go({ name: "clean" })}>
            See suggestions instead
          </Button>
        </div>
      </Sheet>
      {error && <p className="text-body-s text-keep-fg">{error}</p>}
    </div>
  );
}
