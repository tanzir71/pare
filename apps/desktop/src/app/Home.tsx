import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  Download,
  FileText,
  FolderPlus,
  HardDrive,
  Monitor,
  Folder,
  RefreshCw,
  ScanSearch,
  ShieldCheck,
  TrendingUp,
} from "lucide-react";
import { Button, Chip, SearchField, StorageSummary, SuggestionCard, FileRow, cx, type BadgeTone } from "@pare/ui";
import { api, isTauri, on } from "../lib/ipc";
import {
  ago,
  andList,
  categoryDescription,
  categoryTitle,
  formatBytes,
  formatBytesExact,
  growthPhrase,
  protectExplain,
  tierBadge,
} from "../lib/format";
import type { CategorySummary, Finding, Growth, ProtectReason } from "../lib/types";
import { t, tp, useT } from "../i18n";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { useScan, type ScopeKey } from "../state/scan";
import { categoryHue, findingIcon, iconByName } from "../components/icons";
import { Sheet } from "../components/Sheet";

export function cardBadge(c: CategorySummary): { tone: BadgeTone; label: string } {
  if (c.safeBytes > 0 && c.reviewBytes === 0)
    return { tone: "safe", label: c.id === "project-build" ? t("home.badge.rebuilds") : t("tier.safe") };
  if (c.reviewBytes > 0 && c.safeBytes === 0) return { tone: "review", label: t("tier.review") };
  return c.safeBytes >= c.reviewBytes
    ? { tone: "safe", label: t("home.badge.mostlySafe") }
    : { tone: "review", label: t("home.badge.mostlyReview") };
}

function cardMeta(c: CategorySummary): string {
  if (c.safeBytes > 0 && c.reviewBytes > 0) {
    return t("home.cardMeta", { safe: formatBytes(c.safeBytes), review: formatBytes(c.reviewBytes) });
  }
  const unit = c.id === "recycle-bin" ? "count.items" : c.id === "empty-folders" ? "count.folders" : "count.files";
  return tp(unit, c.items);
}

function keptPhrase(protectedCounts: { reason: ProtectReason; count: number }[]): string {
  const n = (rs: ProtectReason[]) => protectedCounts.filter((p) => rs.includes(p.reason)).reduce((a, p) => a + p.count, 0);
  const parts: { text: string; count: number }[] = [];
  const add = (key: "home.kept.uncommitted" | "home.kept.inUse" | "home.kept.recent" | "home.kept.excluded" | "home.kept.protected", count: number) => {
    if (count) parts.push({ text: tp(key, count), count });
  };
  add("home.kept.uncommitted", n(["uncommitted"]));
  add("home.kept.inUse", n(["in-use", "tool-running"]));
  add("home.kept.recent", n(["recently-modified", "changed-since-scan"]));
  add("home.kept.excluded", n(["excluded"]));
  add("home.kept.protected", n(["cloud-only", "system-path", "never-touch"]));
  if (parts.length === 0) return t("home.kept.none");
  const list = andList(parts.map((p) => p.text));
  const single = parts.length === 1 && parts[0]!.count === 1;
  const sentence = t(single ? "home.leftOut.single" : "home.leftOut.multiple", { list });
  return `${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}`;
}

async function safeIds(categories: CategorySummary[]): Promise<string[]> {
  const ids: string[] = [];
  for (const c of categories.filter((c) => c.safeBytes > 0)) {
    const d = await api.categoryGet(c.id);
    for (const f of d.findings) {
      if (f.tier.kind === "safe" && f.disposal.kind !== "guide") ids.push(f.id);
    }
  }
  return ids;
}

export function Home() {
  const { t, tp } = useT();
  const go = useApp((s) => s.go);
  const drives = useApp((s) => s.drives);
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const scan = useScan();
  const clean = useClean();
  const [query, setQuery] = useState("");
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [keptOpen, setKeptOpen] = useState(false);
  const [kept, setKept] = useState<Finding[]>([]);
  const [results, setResults] = useState<Finding[] | null>(null);
  const home = useApp((s) => s.folders?.home);
  const [alerts, setAlerts] = useState<Growth[]>([]);
  const [dismissed, setDismissed] = useState<string[]>([]);
  const monitorOn = settings?.monitor.enabled ?? false;
  useEffect(() => {
    if (!monitorOn) {
      setAlerts([]);
      return;
    }
    let un: (() => void) | undefined;
    let live = true;
    void api.monitorAlerts().then((a) => live && setAlerts(a)).catch(() => {});
    void on("monitor://alerts", (a) => setAlerts(a)).then((u) => (live ? (un = u) : u()));
    return () => {
      live = false;
      un?.();
    };
  }, [monitorOn]);
  const shownAlerts = alerts.filter((a) => !dismissed.includes(a.ruleId)).slice(0, 3);

  const hidden = settings?.hiddenCards ?? [];
  const categories = scan.categories.filter((c) => !hidden.includes(c.id) && c.safeBytes + c.reviewBytes > 0);
  const safe = categories.reduce((a, c) => a + c.safeBytes, 0);
  const review = categories.reduce((a, c) => a + c.reviewBytes, 0);
  const summary = scan.summary;

  // Storage bar for the drives this scan covered.
  const scanned = useMemo(() => {
    const roots = summary?.roots.map((r) => r.path.slice(0, 2).toUpperCase()) ?? [];
    const ds = drives.filter((d) => roots.includes(d.letter.toUpperCase()));
    return ds.length ? ds : drives.filter((d) => d.isFixed);
  }, [summary, drives]);
  const total = scanned.reduce((a, d) => a + d.total, 0);
  const free = scanned.reduce((a, d) => a + d.free, 0);
  const otherUsed = Math.max(0, total - free - safe - review);
  const driveNames = scanned.map((d) => d.letter);
  const overline = summary
    ? t("home.overline", {
        ago: ago(summary.finished),
        drives: driveNames.length > 1 ? t("home.drives", { list: andList(driveNames) }) : t("home.drive", { letter: driveNames[0] ?? "" }),
      })
    : "";

  const chips: { key: ScopeKey; label: string; icon: typeof HardDrive }[] = [
    { key: "pc", label: t("home.scope.pc"), icon: HardDrive },
    { key: "desktop", label: t("home.scope.desktop"), icon: Monitor },
    { key: "downloads", label: t("home.scope.downloads"), icon: Download },
    { key: "documents", label: t("home.scope.documents"), icon: FileText },
    ...(settings?.customRoots ?? []).map((r) => ({ key: `path:${r.path}` as ScopeKey, label: r.label, icon: Folder })),
  ];

  async function addFolder() {
    if (!isTauri) return;
    const picked = await openDialog({ directory: true, multiple: false, title: t("common.dialog.addScanFolder") });
    if (typeof picked !== "string") return;
    const roots = settings?.customRoots ?? [];
    if (!roots.some((r) => r.path === picked)) {
      await updateSettings({ customRoots: [...roots, { path: picked, label: picked }] });
    }
    scan.toggleScope(`path:${picked}`);
  }

  async function freeUp() {
    const ids = await safeIds(categories);
    useClean.getState().confirmSession();
    setConfirmOpen(false);
    await clean.run(ids);
  }

  async function search(q: string) {
    setQuery(q);
    if (!q.trim()) {
      setResults(null);
      return;
    }
    const needle = q.trim().toLowerCase();
    const all: Finding[] = [];
    for (const c of categories) {
      const d = await api.categoryGet(c.id);
      all.push(...d.findings);
    }
    setResults(
      all.filter((f) => f.title.toLowerCase().includes(needle) || f.paths.some((p) => p.toLowerCase().includes(needle))),
    );
  }

  const scopeRow = (
    <div className="flex flex-wrap items-center gap-2" role="group" aria-label={t("home.scope")}>
      <span className="mr-1 text-body-m text-text-secondary">{t("home.scope")}</span>
      {chips.map((c) => (
        <Chip key={c.key} icon={c.icon} selected={scan.scope.includes(c.key)} onClick={() => scan.toggleScope(c.key)}>
          {c.label}
        </Chip>
      ))}
      {isTauri && (
        <Chip icon={FolderPlus} onClick={() => void addFolder()}>
          {t("common.addFolder")}
        </Chip>
      )}
    </div>
  );

  const scopeIsPc = scan.scope.includes("pc");
  const scanLabel = scopeIsPc ? t("common.scanThisPc") : tp("home.scanFolders", scan.scope.length);

  // First run: nothing scanned yet.
  if (!summary && !scan.running) {
    return (
      <div className="mx-auto flex min-h-full max-w-[720px] flex-col items-center justify-center gap-6 px-8 py-12 text-center">
        <span className="flex size-16 items-center justify-center rounded-xl bg-accent-subtle text-accent-on-subtle">
          <ScanSearch size={30} aria-hidden="true" />
        </span>
        <div>
          <h1 className="text-heading-h1">{t("home.firstRun.title")}</h1>
          <p className="mt-2 text-body-l text-text-secondary">{t("home.firstRun.body")}</p>
        </div>
        <div className="flex justify-center">{scopeRow}</div>
        <Button size="l" icon={ScanSearch} onClick={() => void scan.start()}>
          {scanLabel}
        </Button>
        {scan.error && <p className="text-body-s text-keep-fg">{scan.error}</p>}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 pb-10 pl-4 pr-8 pt-6">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="mr-auto text-heading-h1">{t("home.title")}</h1>
        <SearchField
          className="w-[300px]"
          placeholder={t("home.searchPlaceholder")}
          aria-label={t("home.searchLabel")}
          value={query}
          onChange={(e) => void search(e.currentTarget.value)}
        />
        <Button variant="secondary" icon={RefreshCw} onClick={() => void scan.start()} disabled={scan.running}>
          {t("home.scanAgain")}
        </Button>
      </header>

      {scan.running && (
        <button
          type="button"
          onClick={() => go({ name: "scanning" })}
          className="flex items-center gap-2 self-start rounded-full bg-accent-subtle px-3 py-1 text-label-s text-accent-on-subtle"
        >
          <RefreshCw size={14} className="pare-spin" aria-hidden="true" />
          {t("home.scanningPill", { pct: Math.round((scan.progress?.fraction ?? 0) * 100) })}
        </button>
      )}

      {shownAlerts.map((a) => (
        <div key={a.ruleId} role="status" className="flex items-center gap-3 rounded-lg bg-review-bg px-4 py-3 text-body-s text-review-fg">
          <TrendingUp size={18} className="shrink-0" aria-hidden="true" />
          <p className="min-w-0 flex-1">
            <span className="text-body-m-strong">{growthPhrase(a, home)}.</span> {a.label}.
          </p>
          <Button variant="secondary" onClick={() => go({ name: "review", category: a.category })}>
            {t("common.review")}
          </Button>
          <Button variant="ghost" onClick={() => setDismissed((d) => [...d, a.ruleId])}>
            {t("home.dismiss")}
          </Button>
        </div>
      ))}

      {summary && (safe + review > 0 || scan.running) && (
        <StorageSummary
          overline={scan.running ? t("home.scanning") : overline}
          safeBytes={safe}
          reviewBytes={review}
          otherUsedBytes={otherUsed}
          freeBytes={free}
          primaryLabel={safe > 0 ? t("common.freeUp", { size: formatBytes(safe) }) : t("home.nothingSafe")}
          caption={t("home.summaryCaption")}
          secondaryLabel={t("home.reviewAll")}
          reclaimLabel={t("home.canBeFreed")}
          segmentLabels={{
            safe: t("home.segment.safe"),
            review: t("home.segment.review"),
            other: t("home.segment.otherUsed"),
            free: t("home.segment.free"),
          }}
          formatSize={formatBytes}
          formatSizeExact={formatBytesExact}
          onFreeUp={
            safe > 0 && !clean.running
              ? () => (clean.confirmedThisSession ? void freeUp() : setConfirmOpen(true))
              : undefined
          }
          onReviewAll={categories[0] ? () => go({ name: "review", category: categories[0]!.id }) : undefined}
        />
      )}

      {scopeRow}

      {results ? (
        <section aria-label={t("home.searchResults")} className="overflow-hidden rounded-lg border border-border-default">
          {results.length === 0 ? (
            <p className="bg-bg-surface px-4 py-6 text-body-m text-text-secondary">{t("home.noMatches", { query })}</p>
          ) : (
            results.slice(0, 200).map((f) => {
              const b = tierBadge(f.tier, f.badge, f.disposal);
              return (
                <FileRow
                  key={f.id}
                  icon={findingIcon(f)}
                  hue={categoryHue(f.category)}
                  name={f.title}
                  path={f.paths[0] ?? ""}
                  badge={{ tone: b.tone, label: b.label }}
                  age={f.note ?? undefined}
                  size={f.bytes}
                  selectLabel={t("common.selectItem", { name: f.title })}
                  formatSize={formatBytes}
                  formatSizeExact={formatBytesExact}
                  disabled
                  onClick={() => go({ name: "review", category: f.category })}
                  className="cursor-pointer"
                />
              );
            })
          )}
        </section>
      ) : categories.length === 0 && !scan.running ? (
        <div className="flex flex-col items-center gap-3 rounded-xl border border-border-default bg-bg-surface px-8 py-14 text-center">
          <ShieldCheck size={28} className="text-safe-fg" aria-hidden="true" />
          <p className="text-heading-h3">{t("home.empty.title")}</p>
          <p className="text-body-m text-text-secondary">{t("home.empty.body")}</p>
        </div>
      ) : (
        <section aria-labelledby="suggestions-title" className="flex flex-col gap-4">
          <div className="flex items-baseline gap-3">
            <h2 id="suggestions-title" className="text-heading-h3">
              {t("home.suggestions")}
            </h2>
            <span className="text-caption text-text-tertiary">
              {t("home.suggestionsMeta", { categories: tp("count.categories", categories.length) })}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-4 min-[1100px]:grid-cols-3">
            {categories.map((c, i) => (
              <SuggestionCard
                key={c.id}
                className="pare-rise"
                style={{ animationDelay: `${Math.min(i, 8) * 40}ms` }}
                icon={iconByName(c.icon)}
                hue={categoryHue(c.id)}
                title={categoryTitle(c.id)}
                description={categoryDescription(c.id)}
                size={c.safeBytes + c.reviewBytes}
                formatSize={formatBytes}
                formatSizeExact={formatBytesExact}
                meta={cardMeta(c)}
                badge={cardBadge(c)}
                ctaLabel={t("common.review")}
                onCtaClick={() => go({ name: "review", category: c.id })}
              />
            ))}
          </div>
        </section>
      )}

      {summary && !scan.running && (
        <div className="flex items-center gap-4 rounded-lg border border-border-default bg-bg-surface px-5 py-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-md bg-safe-bg text-safe-fg">
            <ShieldCheck size={18} aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-body-m-strong text-text-primary">{t("home.keptTitle")}</p>
            <p className="text-body-s text-text-secondary">{keptPhrase(summary.protected)}</p>
          </div>
          {summary.protected.length > 0 && (
            <Button
              variant="ghost"
              onClick={() => {
                void api.protectedList().then(setKept);
                setKeptOpen(true);
              }}
            >
              {t("home.seeKept")}
            </Button>
          )}
        </div>
      )}

      <Sheet
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title={t("common.freeUpQuestion", { size: formatBytes(safe) })}
        description={t("home.confirm.description")}
        footer={
          <>
            <Button variant="secondary" onClick={() => setConfirmOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={() => void freeUp()}>{t("common.freeUp", { size: formatBytes(safe) })}</Button>
          </>
        }
      >
        <ul className="divide-y divide-border-default">
          {categories
            .filter((c) => c.safeBytes > 0)
            .map((c) => (
              <li key={c.id} className="flex items-center justify-between py-2.5 text-body-m">
                <span>{categoryTitle(c.id)}</span>
                <span className="tabular text-text-secondary">{formatBytes(c.safeBytes)}</span>
              </li>
            ))}
        </ul>
      </Sheet>

      <Sheet
        open={keptOpen}
        onClose={() => setKeptOpen(false)}
        placement="side"
        title={t("home.keptSheet.title")}
        description={t("home.keptSheet.description")}
      >
        <ul className="flex flex-col gap-2">
          {kept.map((f) => (
            <li key={f.id} className="rounded-md border border-border-default px-3 py-2">
              <p className="truncate text-body-m-strong">{f.title}</p>
              <p className="truncate font-mono text-mono-s text-text-tertiary">{f.paths[0]}</p>
              <p className={cx("mt-1 text-caption text-keep-fg")}>
                {f.tier.kind === "protected" ? (f.tier.detail ?? protectExplain(f.tier.reason)) : ""} · {formatBytes(f.bytes)}
              </p>
            </li>
          ))}
          {kept.length === 0 && <li className="text-body-s text-text-secondary">{t("common.loading")}</li>}
        </ul>
        <p className="mt-4 text-caption text-text-tertiary">{tp("count.items", kept.length)}</p>
      </Sheet>
    </div>
  );
}
