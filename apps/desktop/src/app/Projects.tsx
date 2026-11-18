import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderPlus, GitBranch, Package, RefreshCw, Sparkles } from "lucide-react";
import { Badge, Button, CategoryTile, Checkbox, Chip, cx, type BadgeTone } from "@pare/ui";
import { api, isTauri } from "../lib/ipc";
import { ago, andList, formatBytes, orList } from "../lib/format";
import { t, tp, useT, type MessageKey } from "../i18n";
import { isSelectable, totals } from "../lib/selection";
import type { Finding, Project, ProjectStatus, ProjectsView } from "../lib/types";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { Sheet } from "../components/Sheet";

type Filter = "all" | "idle" | "unpushed" | "uncommitted" | "no-remote";

const STATUS_TONE: Record<ProjectStatus, BadgeTone> = {
  "clean-pushed": "safe",
  active: "protected",
  unpushed: "review",
  "no-upstream": "review",
  "no-remote": "review",
  detached: "review",
  uncommitted: "protected",
  "not-git": "neutral",
};

function matches(p: Project, f: Filter, idleDays: number): boolean {
  switch (f) {
    case "all":
      return true;
    case "idle":
      return (p.idleDays ?? 0) >= idleDays;
    case "unpushed":
      return p.status === "unpushed" || p.status === "no-upstream";
    case "uncommitted":
      return p.status === "uncommitted";
    case "no-remote":
      return p.status === "no-remote";
  }
}

function commitLine(p: Project): string {
  const g = p.git;
  if (!g) return t("projects.notInGit");
  const branch = g.branch ?? (g.detached ? t("projects.detached") : "—");
  if (g.dirty) return tp("projects.line.changed", g.dirtyFiles, { branch });
  if (g.ahead > 0) return t("projects.line.unpushed", { branch, n: g.ahead });
  if (!g.hasRemote) return t("projects.line.noRemote", { branch });
  return tp("projects.line.commits", g.commits, { branch });
}

export function Projects() {
  const { t, tp } = useT();
  const settings = useApp((s) => s.settings);
  const update = useApp((s) => s.updateSettings);
  const run = useClean((s) => s.run);
  const cleaning = useClean((s) => s.running);
  const [view, setView] = useState<ProjectsView | null>(null);
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [confirm, setConfirm] = useState<string[] | null>(null);
  const idleDays = settings?.thresholds.idleDays ?? 90;

  async function rescan() {
    setBusy(true);
    try {
      const v = await api.projectsScan();
      setView(v);
      setSel(new Set(v.findings.filter((f) => f.tier.kind === "safe").map((f) => f.id)));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void api.projectsList().then((v) => {
      if (v.checked) {
        setView(v);
        setSel(new Set(v.findings.filter((f) => f.tier.kind === "safe").map((f) => f.id)));
      } else {
        void rescan();
      }
    });
  }, []);

  const findingById = useMemo(() => new Map((view?.findings ?? []).map((f) => [f.id, f])), [view]);
  const projects = useMemo(
    () => [...(view?.projects ?? [])].sort((a, b) => b.bytes - a.bytes || a.name.localeCompare(b.name)),
    [view],
  );
  const visible = projects.filter((p) => matches(p, filter, idleDays));
  const safe = (view?.findings ?? []).filter((f) => f.tier.kind === "safe");
  const safeBytes = safe.reduce((a, f) => a + f.bytes, 0);
  const sum = totals(sel, view?.findings ?? []);
  // Rebuild commands come from each project, not the finding's (translated) "Rebuild with" fact.
  const safeIds = new Set(safe.map((f) => f.id));
  const rebuildHints = [
    ...new Set(
      (view?.projects ?? [])
        .filter((p) => p.findingId && safeIds.has(p.findingId) && p.rebuild.length > 0)
        .map((p) => p.rebuild.join(" · ")),
    ),
  ].slice(0, 2);
  const rootsText = andList(view?.roots ?? []);

  const counts: Record<Filter, number> = {
    all: projects.length,
    idle: projects.filter((p) => matches(p, "idle", idleDays)).length,
    unpushed: projects.filter((p) => matches(p, "unpushed", idleDays)).length,
    uncommitted: projects.filter((p) => matches(p, "uncommitted", idleDays)).length,
    "no-remote": projects.filter((p) => matches(p, "no-remote", idleDays)).length,
  };
  const filters: { key: Filter; label: string }[] = [
    { key: "all", label: t("projects.filter.all") },
    { key: "idle", label: t("projects.filter.idle", { days: idleDays }) },
    { key: "unpushed", label: t("projects.filter.unpushed") },
    { key: "uncommitted", label: t("projects.filter.uncommitted") },
    { key: "no-remote", label: t("projects.filter.no-remote") },
  ];

  const visibleFindings: Finding[] = visible.flatMap((p) => (p.findingId && findingById.get(p.findingId) ? [findingById.get(p.findingId)!] : []));
  const selectableVisible = visibleFindings.filter(isSelectable);
  const allChecked = selectableVisible.length > 0 && selectableVisible.every((f) => sel.has(f.id));
  const someChecked = selectableVisible.some((f) => sel.has(f.id));

  function freeUp(ids: string[]) {
    const chosen = (view?.findings ?? []).filter((f) => ids.includes(f.id) && isSelectable(f));
    if (chosen.some((f) => f.tier.kind !== "safe")) setConfirm(chosen.map((f) => f.id));
    else void run(chosen.map((f) => f.id));
  }

  async function addFolder() {
    if (!isTauri || !settings) return;
    const picked = await openDialog({ directory: true, multiple: false, title: t("common.dialog.addProjectsFolder") });
    if (typeof picked !== "string") return;
    if (!settings.projectRoots.includes(picked)) await update({ projectRoots: [...settings.projectRoots, picked] });
    await rescan();
  }

  const gitCount = projects.filter((p) => p.git).length;

  return (
    <div className="flex flex-col gap-5 pb-24 pl-4 pr-8 pt-6">
      <header className="flex items-start gap-3">
        <div className="mr-auto min-w-0">
          <h1 className="text-heading-h1">{t("projects.title")}</h1>
          <p className="truncate text-body-s text-text-secondary">
            {view?.checked
              ? rootsText
                ? t("projects.summaryIn", { repos: tp("projects.repos", gitCount), roots: rootsText, ago: ago(view.checked) })
                : t("projects.summary", { repos: tp("projects.repos", gitCount), ago: ago(view.checked) })
              : busy
                ? t("projects.looking")
                : t("projects.noneYet")}
          </p>
        </div>
        <Button variant="secondary" icon={RefreshCw} onClick={() => void rescan()} disabled={busy}>
          {busy ? t("common.checking") : t("projects.checkAgain")}
        </Button>
        {isTauri && (
          <Button variant="secondary" icon={FolderPlus} onClick={() => void addFolder()}>
            {t("common.addFolder")}
          </Button>
        )}
      </header>

      <div className="flex flex-wrap gap-2" role="group" aria-label={t("projects.filterLabel")}>
        {filters.map((f) => (
          <Chip key={f.key} selected={filter === f.key} onClick={() => setFilter(f.key)}>
            {f.label} · {counts[f.key]}
          </Chip>
        ))}
      </div>

      {safe.length > 0 && (
        <div className="flex items-center gap-4 rounded-lg border border-border-default bg-bg-surface px-5 py-4">
          <CategoryTile icon={Package} hue="orange" size="md" />
          <div className="min-w-0 flex-1">
            <p className="text-title-m">{tp("projects.bulk.title", safe.length, { size: formatBytes(safeBytes) })}</p>
            <p className="text-body-s text-text-secondary">
              {t("projects.bulk.body", { days: idleDays })}
              {rebuildHints.length > 0 && ` ${t("projects.bulk.rebuild", { commands: orList(rebuildHints) })}`}
            </p>
          </div>
          <Button icon={Sparkles} onClick={() => freeUp(safe.map((f) => f.id))} disabled={cleaning}>
            {tp("projects.clean", safe.length)}
          </Button>
        </div>
      )}

      {view && projects.length === 0 && !busy && (
        <div className="flex flex-col items-center gap-2 rounded-xl border border-border-default bg-bg-surface px-8 py-14 text-center">
          <GitBranch size={28} className="text-text-tertiary" aria-hidden="true" />
          <p className="text-heading-h3">{t("projects.empty.title")}</p>
          <p className="max-w-[460px] text-body-m text-text-secondary">{t("projects.empty.body")}</p>
        </div>
      )}

      {visible.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-border-default bg-bg-surface" role="table" aria-label={t("projects.table")}>
          <div role="row" className="grid grid-cols-[40px_minmax(0,2.2fr)_minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1.2fr)_90px] items-center gap-3 border-b border-border-default bg-bg-subtle px-4 py-2.5 text-overline uppercase text-text-tertiary">
            <span role="columnheader">
              <Checkbox
                aria-label={t("projects.selectAll")}
                checked={allChecked}
                indeterminate={!allChecked && someChecked}
                disabled={selectableVisible.length === 0}
                onChange={() =>
                  setSel((s) => {
                    const next = new Set(s);
                    for (const f of selectableVisible) {
                      if (allChecked) next.delete(f.id);
                      else next.add(f.id);
                    }
                    return next;
                  })
                }
              />
            </span>
            <span role="columnheader">{t("projects.col.project")}</span>
            <span role="columnheader">{t("projects.col.lastCommit")}</span>
            <span role="columnheader">{t("projects.col.gitStatus")}</span>
            <span role="columnheader">{t("projects.col.buildFiles")}</span>
            <span role="columnheader" className="text-right">
              {t("projects.col.size")}
            </span>
          </div>
          {visible.map((p) => {
            const f = p.findingId ? findingById.get(p.findingId) : undefined;
            const selectable = !!f && isSelectable(f);
            const statusKey: MessageKey = `projects.status.${p.status}`;
            const protectedRow = p.tier.kind === "protected";
            return (
              <div
                key={p.root}
                role="row"
                className="grid grid-cols-[40px_minmax(0,2.2fr)_minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1.2fr)_90px] items-center gap-3 border-b border-border-default px-4 py-3 last:border-b-0"
              >
                <span role="cell">
                  <Checkbox
                    aria-label={t("common.selectItem", { name: p.name })}
                    checked={!!f && sel.has(f.id)}
                    disabled={!selectable}
                    onChange={() =>
                      f &&
                      setSel((s) => {
                        const next = new Set(s);
                        if (next.has(f.id)) next.delete(f.id);
                        else next.add(f.id);
                        return next;
                      })
                    }
                  />
                </span>
                <span role="cell" className="flex min-w-0 items-center gap-3">
                  <CategoryTile icon={GitBranch} hue={protectedRow ? "slate" : "orange"} size="sm" />
                  <span className="min-w-0">
                    <span className="block truncate text-body-m-strong">{p.name}</span>
                    <span className="block truncate font-mono text-mono-s text-text-tertiary" title={p.root}>
                      {p.root}
                    </span>
                  </span>
                </span>
                <span role="cell" className="min-w-0">
                  <span className="block text-body-m">{ago(p.git?.lastCommit ?? p.lastChange)}</span>
                  <span className="block truncate text-caption text-text-tertiary">{commitLine(p)}</span>
                </span>
                <span role="cell">
                  <Badge tone={STATUS_TONE[p.status]}>{t(statusKey)}</Badge>
                </span>
                <span role="cell" className="truncate text-body-s text-text-secondary" title={p.artifacts.map((a) => a.path).join("\n")}>
                  {p.artifacts.length ? [...new Set(p.artifacts.map((a) => a.name))].join(", ") : "—"}
                </span>
                <span role="cell" className={cx("tabular text-right text-body-m-strong", !p.bytes && "text-text-tertiary")}>
                  {p.bytes ? formatBytes(p.bytes) : "—"}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {sum.count > 0 && (
        <div
          role="region"
          aria-label={t("common.selection")}
          className="pare-rise fixed bottom-6 left-[calc(50%+120px)] z-10 flex w-max -translate-x-1/2 items-center gap-6 whitespace-nowrap rounded-lg bg-bg-inverse py-2.5 pl-5 pr-2.5 shadow-floating"
        >
          <div className="tabular">
            <p className="text-body-m-strong text-on-inverse-text">
              {tp("count.projects", sum.count)} · {formatBytes(sum.bytes)}
            </p>
            <p className="text-caption text-on-inverse-text-muted">
              {t("common.selectionSplit", { now: formatBytes(sum.freesNow), bin: formatBytes(sum.toRecycleBin) })}
            </p>
          </div>
          <button type="button" onClick={() => setSel(new Set())} className="rounded-md px-3 py-2 text-label-m text-on-inverse-text-muted hover:text-on-inverse-text">
            {t("common.clear")}
          </button>
          <Button onClick={() => freeUp([...sel])} disabled={cleaning}>
            {cleaning ? t("common.freeingUp") : t("common.freeUp", { size: formatBytes(sum.bytes) })}
          </Button>
        </div>
      )}

      <Sheet
        open={confirm !== null}
        onClose={() => setConfirm(null)}
        title={tp("projects.confirm.title", confirm?.length ?? 0)}
        description={t("projects.confirm.description")}
        footer={
          <>
            <Button variant="secondary" onClick={() => setConfirm(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              onClick={() => {
                const ids = confirm ?? [];
                setConfirm(null);
                void run(ids);
              }}
            >
              {t("projects.confirm.action", { n: confirm?.length ?? 0 })}
            </Button>
          </>
        }
      />
    </div>
  );
}
