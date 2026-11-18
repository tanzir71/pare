import { useState } from "react";
import { FolderOpen, Play, ShieldAlert, Trash2, X, Sparkles } from "lucide-react";
import { Badge, Button, CategoryTile, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { ago, comesBackLabel, date, formatBytes, formatCount, protectExplain, tierBadge, tildePath } from "../lib/format";
import { t, tp, useT } from "../i18n";
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
  const { t } = useT();
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
      <h3 className="text-body-m-strong text-text-primary">{t("guide.title")}</h3>
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
                          <ShieldAlert size={14} aria-hidden="true" /> {t("guide.needsAdmin")}
                        </p>
                        <p className="mt-1">{t("guide.adminHint")}</p>
                        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all font-mono text-mono-s text-text-primary">{cmd}</pre>
                        <div className="mt-2 flex gap-2">
                          <Button variant="secondary" onClick={() => void navigator.clipboard?.writeText(cmd)}>
                            {t("guide.copyCommand")}
                          </Button>
                          <Button variant="ghost" onClick={() => void api.relaunchElevated().catch((e) => setResults((x) => ({ ...x, [idx]: String(e) })))}>
                            {t("common.restartAsAdmin")}
                          </Button>
                        </div>
                      </div>
                    ) : confirm === idx ? (
                      <div className="rounded-md border border-border-default px-3 py-2">
                        <p className="text-text-primary">{t("guide.confirm")}</p>
                        <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all font-mono text-mono-s">{cmd}</pre>
                        <div className="mt-2 flex gap-2">
                          <Button onClick={() => void run(idx)}>{t("guide.run")}</Button>
                          <Button variant="ghost" onClick={() => setConfirm(null)}>
                            {t("common.cancel")}
                          </Button>
                        </div>
                      </div>
                    ) : (
                      <div>
                        <Button variant="secondary" icon={Play} disabled={running !== null} onClick={() => setConfirm(idx)}>
                          {running === idx ? t("guide.running") : a.label}
                        </Button>
                      </div>
                    )}
                    {typeof res === "string" && <p className="text-keep-fg">{res}</p>}
                    {res && typeof res !== "string" && (
                      <div className={cx("rounded-md px-3 py-2", res.ok ? "bg-safe-bg text-safe-fg" : "bg-review-bg text-review-fg")}>
                        <p>
                          {res.ok ? t("guide.done") : t("guide.failed")}
                          {res.before > res.after ? ` ${t("guide.freed", { size: formatBytes(res.before - res.after) })}` : ""}
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
      <p className="mt-2 text-caption text-text-tertiary">{t("guide.footer")}</p>
    </section>
  );
}

export function disposalAction(f: Finding): { label: string; icon: typeof Trash2 } | null {
  switch (f.disposal.kind) {
    case "recycle":
      return { label: t("common.moveToRecycleBin"), icon: Trash2 };
    case "delete":
      return { label: t("common.freeUp", { size: formatBytes(f.bytes) }), icon: Sparkles };
    case "command":
      return { label: t("details.action.run", { command: [f.disposal.program, ...f.disposal.args].join(" ") }), icon: Play };
    case "empty-recycle-bin":
      return { label: t("common.emptyNow", { size: formatBytes(f.bytes) }), icon: Trash2 };
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

function gitStatus(g: NonNullable<Finding["git"]>): string {
  if (g.dirty) return t("details.git.uncommitted");
  if (g.ahead > 0) return tp("details.git.unpushed", g.ahead);
  if (!g.hasRemote) return t("details.git.noRemote");
  return t("details.git.cleanPushed");
}

export function DetailsPanel({ finding: f, onClose, onDispose, onKeep, busy }: Props) {
  const { t, tp } = useT();
  const home = useApp((s) => s.folders?.home);
  const b = tierBadge(f.tier, f.badge, f.disposal);
  const action = disposalAction(f);
  const protectedTier = f.tier.kind === "protected" ? f.tier : null;
  const facts: [string, string][] = [
    ...(f.createdBy ? [[t("details.fact.createdBy"), f.createdBy] as [string, string]] : []),
    [t("details.fact.created"), date(f.dates.createdMin)],
    [t("details.fact.lastModified"), date(f.dates.modifiedMax)],
    ...(f.dates.accessedMax ? [[t("details.fact.lastOpened"), ago(f.dates.accessedMax)] as [string, string]] : []),
    [t("details.fact.comesBack"), comesBackLabel(f.comesBack)],
    ...(f.git
      ? ([
          [t("details.fact.branch"), f.git.branch ?? (f.git.detached ? t("details.git.detached") : "—")],
          [t("details.fact.lastCommit"), ago(f.git.lastCommit)],
          [t("details.fact.gitStatus"), gitStatus(f.git)],
        ] as [string, string][])
      : []),
    ...f.facts,
  ];
  const location = f.disposal.kind === "empty-recycle-bin" ? f.disposal.drive : (f.paths[0] ?? f.sourcePath ?? "");

  return (
    <div className="flex flex-col gap-5 p-5">
      <div className="flex items-center justify-between">
        <span className="text-body-m text-text-secondary">{t("details.title")}</span>
        <button type="button" aria-label={t("details.close")} onClick={onClose} className={cx("-mr-1 rounded-md p-1.5 text-text-secondary hover:bg-bg-subtle", focusRing)}>
          <X size={18} aria-hidden="true" />
        </button>
      </div>

      <div className="flex items-start gap-3">
        <CategoryTile icon={findingIcon(f)} hue={categoryHue(f.category)} size="lg" />
        <div className="min-w-0">
          <h2 className="text-title-m text-text-primary">{f.title}</h2>
          <p className="tabular text-body-s text-text-secondary">
            {f.items === 0 ? t("details.empty") : tp("count.files", f.items)}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <span className="tabular text-heading-h1 text-text-primary" title={t("details.bytesOnDisk", { bytes: formatCount(f.bytes) })}>
          {f.bytes === 0 && f.disposal.kind === "guide" ? "—" : formatBytes(f.bytes)}
        </span>
        <Badge tone={b.tone} srDescription={b.sr}>
          {b.label}
        </Badge>
      </div>

      {protectedTier && (
        <p className="rounded-md bg-keep-bg px-3 py-2 text-body-s text-keep-fg">
          {t("details.kept", { reason: protectedTier.detail ?? protectExplain(protectedTier.reason) })}
        </p>
      )}

      <section>
        <h3 className="text-body-m-strong text-text-primary">{t("details.why")}</h3>
        <p className="mt-1 text-body-s text-text-secondary">{f.why}</p>
      </section>
      <section>
        <h3 className="text-body-m-strong text-text-primary">{t("details.ifRemoved")}</h3>
        <p className="mt-1 text-body-s text-text-secondary">{f.ifRemoved}</p>
      </section>

      {f.disposal.kind === "guide" && <GuideSteps key={f.id} finding={f} />}
      {f.disposal.kind === "command" && (
        <p className="text-body-s text-text-secondary">
          {t("details.command")}{" "}
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
          <dt className="w-[108px] shrink-0 text-text-tertiary">{t("details.location")}</dt>
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
            {t("common.openInExplorer")}
          </Button>
        )}
        {f.paths[0] && f.tier.kind !== "protected" && (
          <Button variant="ghost" fullWidth onClick={() => void onKeep(f.paths[0]!)}>
            {t("details.alwaysKeep")}
          </Button>
        )}
      </div>
    </div>
  );
}
