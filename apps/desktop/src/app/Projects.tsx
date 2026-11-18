import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderPlus, GitBranch, Package, RefreshCw, Sparkles } from "lucide-react";
import { Badge, Button, CategoryTile, Checkbox, Chip, cx, type BadgeTone } from "@pare/ui";
import { api, isTauri } from "../lib/ipc";
import { ago, formatBytes, formatCount, plural } from "../lib/format";
import { isSelectable, totals } from "../lib/selection";
import type { Finding, Project, ProjectStatus, ProjectsView } from "../lib/types";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { Sheet } from "../components/Sheet";

type Filter = "all" | "idle" | "unpushed" | "uncommitted" | "no-remote";

const STATUS: Record<ProjectStatus, { label: string; tone: BadgeTone }> = {
  "clean-pushed": { label: "Clean & pushed", tone: "safe" },
  active: { label: "Active", tone: "protected" },
  unpushed: { label: "Unpushed commits", tone: "review" },
  "no-upstream": { label: "Not pushed", tone: "review" },
  "no-remote": { label: "No remote", tone: "review" },
  detached: { label: "Detached HEAD", tone: "review" },
  uncommitted: { label: "Uncommitted", tone: "protected" },
  "not-git": { label: "Not in git", tone: "neutral" },
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
  if (!g) return "not in git";
  const branch = g.branch ?? (g.detached ? "detached" : "—");
  if (g.dirty) return `${branch} · ${plural(g.dirtyFiles, "file")} changed`;
  if (g.ahead > 0) return `${branch} · ${g.ahead} unpushed`;
  if (!g.hasRemote) return `${branch} · no remote`;
  return `${branch} · ${plural(g.commits, "commit")}`;
}

export function Projects() {
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
  const t = totals(sel, view?.findings ?? []);
  const rebuildHints = [...new Set(safe.flatMap((f) => f.facts.filter(([k]) => k === "Rebuild with").map(([, v]) => v)))].slice(0, 2);
  const rootsText = (view?.roots ?? []).join(" and ");

  const counts: Record<Filter, number> = {
    all: projects.length,
    idle: projects.filter((p) => matches(p, "idle", idleDays)).length,
    unpushed: projects.filter((p) => matches(p, "unpushed", idleDays)).length,
    uncommitted: projects.filter((p) => matches(p, "uncommitted", idleDays)).length,
    "no-remote": projects.filter((p) => matches(p, "no-remote", idleDays)).length,
  };
  const filters: { key: Filter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "idle", label: `Idle ${idleDays}+ days` },
    { key: "unpushed", label: "Unpushed" },
    { key: "uncommitted", label: "Uncommitted" },
    { key: "no-remote", label: "No remote" },
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
    const picked = await openDialog({ directory: true, multiple: false, title: "Add a projects folder" });
    if (typeof picked !== "string") return;
    if (!settings.projectRoots.includes(picked)) await update({ projectRoots: [...settings.projectRoots, picked] });
    await rescan();
  }

  const gitCount = projects.filter((p) => p.git).length;

  return (
    <div className="flex flex-col gap-5 pb-24 pl-4 pr-8 pt-6">
      <header className="flex items-start gap-3">
        <div className="mr-auto min-w-0">
          <h1 className="text-heading-h1">Projects</h1>
          <p className="truncate text-body-s text-text-secondary">
            {view?.checked
              ? `${plural(gitCount, "Git repository", "Git repositories")}${rootsText ? ` in ${rootsText}` : ""} · checked ${ago(view.checked)}`
              : busy
                ? "Looking for projects…"
                : "No project folders found yet"}
          </p>
        </div>
        <Button variant="secondary" icon={RefreshCw} onClick={() => void rescan()} disabled={busy}>
          {busy ? "Checking…" : "Check again"}
        </Button>
        {isTauri && (
          <Button variant="secondary" icon={FolderPlus} onClick={() => void addFolder()}>
            Add folder
          </Button>
        )}
      </header>

      <div className="flex flex-wrap gap-2" role="group" aria-label="Filter projects">
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
            <p className="text-title-m">
              Clean build files in {plural(safe.length, "idle project")} · {formatBytes(safeBytes)}
            </p>
            <p className="text-body-s text-text-secondary">
              Only repos that are committed, pushed and idle {idleDays}+ days.
              {rebuildHints.length > 0 && ` Rebuild anytime with ${rebuildHints.join(" or ")}.`}
            </p>
          </div>
          <Button icon={Sparkles} onClick={() => freeUp(safe.map((f) => f.id))} disabled={cleaning}>
            Clean {plural(safe.length, "project")}
          </Button>
        </div>
      )}

      {view && projects.length === 0 && !busy && (
        <div className="flex flex-col items-center gap-2 rounded-xl border border-border-default bg-bg-surface px-8 py-14 text-center">
          <GitBranch size={28} className="text-text-tertiary" aria-hidden="true" />
          <p className="text-heading-h3">No projects found</p>
          <p className="max-w-[460px] text-body-m text-text-secondary">
            Pare looks in ~\source, ~\code, ~\Projects, Documents\GitHub and D:\code. Add the folder where you keep your code.
          </p>
        </div>
      )}

      {visible.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-border-default bg-bg-surface" role="table" aria-label="Projects">
          <div role="row" className="grid grid-cols-[40px_minmax(0,2.2fr)_minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1.2fr)_90px] items-center gap-3 border-b border-border-default bg-bg-subtle px-4 py-2.5 text-overline uppercase text-text-tertiary">
            <span role="columnheader">
              <Checkbox
                aria-label="Select all shown projects"
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
            <span role="columnheader">Project</span>
            <span role="columnheader">Last commit</span>
            <span role="columnheader">Git status</span>
            <span role="columnheader">Build files</span>
            <span role="columnheader" className="text-right">
              Size
            </span>
          </div>
          {visible.map((p) => {
            const f = p.findingId ? findingById.get(p.findingId) : undefined;
            const selectable = !!f && isSelectable(f);
            const st = STATUS[p.status];
            const protectedRow = p.tier.kind === "protected";
            return (
              <div
                key={p.root}
                role="row"
                className="grid grid-cols-[40px_minmax(0,2.2fr)_minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1.2fr)_90px] items-center gap-3 border-b border-border-default px-4 py-3 last:border-b-0"
              >
                <span role="cell">
                  <Checkbox
                    aria-label={`Select ${p.name}`}
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
                  <Badge tone={st.tone}>{st.label}</Badge>
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

      {t.count > 0 && (
        <div
          role="region"
          aria-label="Selection"
          className="pare-rise fixed bottom-6 left-[calc(50%+120px)] z-10 flex w-max -translate-x-1/2 items-center gap-6 whitespace-nowrap rounded-lg bg-bg-inverse py-2.5 pl-5 pr-2.5 shadow-floating"
        >
          <div className="tabular">
            <p className="text-body-m-strong text-on-inverse-text">
              {plural(t.count, "project")} · {formatBytes(t.bytes)}
            </p>
            <p className="text-caption text-on-inverse-text-muted">
              {formatBytes(t.freesNow)} removed now · {formatBytes(t.toRecycleBin)} to Recycle Bin
            </p>
          </div>
          <button type="button" onClick={() => setSel(new Set())} className="rounded-md px-3 py-2 text-label-m text-on-inverse-text-muted hover:text-on-inverse-text">
            Clear
          </button>
          <Button onClick={() => freeUp([...sel])} disabled={cleaning}>
            {cleaning ? "Freeing up…" : `Free up ${formatBytes(t.bytes)}`}
          </Button>
        </div>
      )}

      <Sheet
        open={confirm !== null}
        onClose={() => setConfirm(null)}
        title={`Clean ${plural(confirm?.length ?? 0, "project")}?`}
        description="Some of these projects aren't idle, pushed or in git. Their build files are removed (or recycled outside git); your code stays."
        footer={
          <>
            <Button variant="secondary" onClick={() => setConfirm(null)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                const ids = confirm ?? [];
                setConfirm(null);
                void run(ids);
              }}
            >
              Clean {formatCount(confirm?.length ?? 0)}
            </Button>
          </>
        }
      />
    </div>
  );
}
