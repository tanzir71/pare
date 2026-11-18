import { useState } from "react";
import { FolderOpen, Play, ShieldAlert, Trash2, X, Sparkles } from "lucide-react";
import { Badge, Button, CategoryTile, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { ago, comesBackLabel, date, formatBytes, formatCount, protectExplain, tierBadge, tildePath } from "../lib/format";
import type { ActionReport, Finding, GuideAction } from "../lib/types";
import { useApp } from "../state/app";
import { categoryHue, findingIcon } from "../components/icons";

/** The command a guide action runs, as the user would type it in a terminal. */
export function actionCommand(a: GuideAction, paths: string[]): string {
  if (a.run[0] === "pare:compact-vhd") {
    return paths
      .map((p) => ["diskpart", `select vdisk file="${p}"`, "attach vdisk readonly", "compact vdisk", "detach vdisk", "exit"].join("\n"))
      .join("\n\n");
  }
  return a.run.join(" ");
}

function GuideSteps({ finding: f }: { finding: Finding }) {
  const elevated = useApp((s) => s.info?.elevated ?? false);
  const [confirm, setConfirm] = useState<number | null>(null);
  const [running, setRunning] = useState<number | null>(null);
  const [results, setResults] = useState<Record<number, ActionReport | string>>({});
  if (f.disposal.kind !== "guide") return null;
  const { steps, actions } = f.disposal;

  async function run(idx: number) {
    setConfirm(null);
    setRunning(idx);
    try {
      const r = await api.guideRun(f.id, idx);
      setResults((x) => ({ ...x, [idx]: r }));
    } catch (e) {
      setResults((x) => ({ ...x, [idx]: String(e) }));
    } finally {
      setRunning(null);
    }
  }

  return (
    <section>
      <h3 className="text-body-m-strong text-text-primary">How to clean it up</h3>
      <ol className="mt-1 list-decimal space-y-3 pl-5 text-body-s text-text-secondary">
        {steps.map((s, i) => (
          <li key={s}>
            <p>{s}</p>
            {actions
              .map((a, idx) => ({ a, idx }))
              .filter(({ a }) => a.step === i)
              .map(({ a, idx }) => {
                const res = results[idx];
                const cmd = actionCommand(a, f.paths);
                return (
                  <div key={idx} className="mt-2 flex flex-col gap-2">
                    {a.admin && !elevated ? (
                      <div className="rounded-md bg-bg-subtle px-3 py-2">
                        <p className="flex items-center gap-1.5 text-text-primary">
                          <ShieldAlert size={14} aria-hidden="true" /> Needs administrator rights
                        </p>
                        <p className="mt-1">Run this in an administrator terminal, or restart Pare as administrator and come back here.</p>
                        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all font-mono text-mono-s text-text-primary">{cmd}</pre>
                        <div className="mt-2 flex gap-2">
                          <Button variant="secondary" onClick={() => void navigator.clipboard?.writeText(cmd)}>
                            Copy command
                          </Button>
                          <Button variant="ghost" onClick={() => void api.relaunchElevated().catch((e) => setResults((x) => ({ ...x, [idx]: String(e) })))}>
                            Restart as administrator
                          </Button>
                        </div>
                      </div>
                    ) : confirm === idx ? (
                      <div className="rounded-md border border-border-default px-3 py-2">
                        <p className="text-text-primary">Run this now?</p>
                        <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all font-mono text-mono-s">{cmd}</pre>
                        <div className="mt-2 flex gap-2">
                          <Button onClick={() => void run(idx)}>Run</Button>
                          <Button variant="ghost" onClick={() => setConfirm(null)}>
                            Cancel
                          </Button>
                        </div>
                      </div>
                    ) : (
                      <div>
                        <Button variant="secondary" icon={Play} disabled={running !== null} onClick={() => setConfirm(idx)}>
                          {running === idx ? "Running…" : a.label}
                        </Button>
                      </div>
                    )}
                    {typeof res === "string" && <p className="text-keep-fg">{res}</p>}
                    {res && typeof res !== "string" && (
                      <div className={cx("rounded-md px-3 py-2", res.ok ? "bg-safe-bg text-safe-fg" : "bg-review-bg text-review-fg")}>
                        <p>
                          {res.ok ? "Done." : "That didn't work."}
                          {res.before > res.after ? ` Freed ${formatBytes(res.before - res.after)}.` : ""}
                        </p>
                        {res.output && <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap font-mono text-mono-s">{res.output}</pre>}
                      </div>
                    )}
                  </div>
                );
              })}
          </li>
        ))}
      </ol>
      <p className="mt-2 text-caption text-text-tertiary">Pare never deletes this itself. Each step runs only when you click it, and is written to the log.</p>
    </section>
  );
}

export function disposalAction(f: Finding): { label: string; icon: typeof Trash2 } | null {
  switch (f.disposal.kind) {
    case "recycle":
      return { label: "Move to Recycle Bin", icon: Trash2 };
    case "delete":
      return { label: `Free up ${formatBytes(f.bytes)}`, icon: Sparkles };
    case "command":
      return { label: `Run ${[f.disposal.program, ...f.disposal.args].join(" ")}`, icon: Play };
    case "empty-recycle-bin":
      return { label: `Empty ${formatBytes(f.bytes)} now`, icon: Trash2 };
    case "guide":
      return null;
  }
}

interface Props {
  finding: Finding;
  onClose: () => void;
  onDispose: () => void;
  onKeep: (path: string) => void | Promise<void>;
  busy?: boolean;
}

export function DetailsPanel({ finding: f, onClose, onDispose, onKeep, busy }: Props) {
  const home = useApp((s) => s.folders?.home);
  const b = tierBadge(f.tier, f.badge, f.disposal);
  const action = disposalAction(f);
  const protectedTier = f.tier.kind === "protected" ? f.tier : null;
  const facts: [string, string][] = [
    ...(f.createdBy ? [["Created by", f.createdBy] as [string, string]] : []),
    ["Created", date(f.dates.createdMin)],
    ["Last modified", date(f.dates.modifiedMax)],
    ...(f.dates.accessedMax ? [["Last opened", ago(f.dates.accessedMax)] as [string, string]] : []),
    ["Comes back?", comesBackLabel(f.comesBack)],
    ...(f.git
      ? ([
          ["Branch", f.git.branch ?? (f.git.detached ? "Detached HEAD" : "—")],
          ["Last commit", ago(f.git.lastCommit)],
          ["Git status", f.git.dirty ? "Uncommitted changes" : f.git.ahead > 0 ? `${f.git.ahead} unpushed commits` : !f.git.hasRemote ? "No remote" : "Clean & pushed"],
        ] as [string, string][])
      : []),
    ...f.facts,
  ];
  const location = f.disposal.kind === "empty-recycle-bin" ? f.disposal.drive : (f.paths[0] ?? f.sourcePath ?? "");

  return (
    <div className="flex flex-col gap-5 p-5">
      <div className="flex items-center justify-between">
        <span className="text-body-m text-text-secondary">Details</span>
        <button type="button" aria-label="Close details" onClick={onClose} className={cx("-mr-1 rounded-md p-1.5 text-text-secondary hover:bg-bg-subtle", focusRing)}>
          <X size={18} aria-hidden="true" />
        </button>
      </div>

      <div className="flex items-start gap-3">
        <CategoryTile icon={findingIcon(f)} hue={categoryHue(f.category)} size="lg" />
        <div className="min-w-0">
          <h2 className="text-title-m text-text-primary">{f.title}</h2>
          <p className="tabular text-body-s text-text-secondary">
            {f.items === 0 ? "Empty" : `${formatCount(f.items)} ${f.items === 1 ? "file" : "files"}`}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <span className="tabular text-heading-h1 text-text-primary" title={`${f.bytes.toLocaleString("en-US")} bytes on disk`}>
          {f.bytes === 0 && f.disposal.kind === "guide" ? "—" : formatBytes(f.bytes)}
        </span>
        <Badge tone={b.tone} srDescription={b.sr}>
          {b.label}
        </Badge>
      </div>

      {protectedTier && (
        <p className="rounded-md bg-keep-bg px-3 py-2 text-body-s text-keep-fg">
          Kept: {protectedTier.detail ?? protectExplain(protectedTier.reason)}. It can't be selected until that changes.
        </p>
      )}

      <section>
        <h3 className="text-body-m-strong text-text-primary">Why Pare suggests this</h3>
        <p className="mt-1 text-body-s text-text-secondary">{f.why}</p>
      </section>
      <section>
        <h3 className="text-body-m-strong text-text-primary">If you remove it</h3>
        <p className="mt-1 text-body-s text-text-secondary">{f.ifRemoved}</p>
      </section>

      {f.disposal.kind === "guide" && <GuideSteps key={f.id} finding={f} />}
      {f.disposal.kind === "command" && (
        <p className="text-body-s text-text-secondary">
          Pare runs the tool's own cleanup:{" "}
          <code className="font-mono text-mono-s text-text-primary">{[f.disposal.program, ...f.disposal.args].join(" ")}</code>
        </p>
      )}

      <dl className="overflow-hidden rounded-md border border-border-default text-body-s">
        {facts.map(([k, v]) => (
          <div key={k} className="flex gap-3 border-b border-border-default px-3 py-2 last:border-b-0">
            <dt className="w-[108px] shrink-0 text-text-tertiary">{k}</dt>
            <dd className="min-w-0 text-text-primary">{v}</dd>
          </div>
        ))}
        <div className="flex gap-3 px-3 py-2">
          <dt className="w-[108px] shrink-0 text-text-tertiary">Location</dt>
          <dd className="min-w-0 break-all font-mono text-mono-s text-text-primary" title={location}>
            {tildePath(location, home)}
          </dd>
        </div>
      </dl>

      <div className="flex flex-col gap-2">
        {action && !protectedTier && (
          <Button icon={action.icon} fullWidth onClick={onDispose} disabled={busy}>
            {action.label}
          </Button>
        )}
        {f.paths[0] && (
          <Button variant="secondary" icon={FolderOpen} fullWidth onClick={() => void api.openInExplorer(f.paths[0]!)}>
            Open in Explorer
          </Button>
        )}
        {f.paths[0] && f.tier.kind !== "protected" && (
          <Button variant="ghost" fullWidth onClick={() => void onKeep(f.paths[0]!)}>
            Always keep this
          </Button>
        )}
      </div>
    </div>
  );
}
