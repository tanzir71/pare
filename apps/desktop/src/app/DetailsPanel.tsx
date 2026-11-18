import { FolderOpen, Play, Trash2, X, Sparkles } from "lucide-react";
import { Badge, Button, CategoryTile, cx, focusRing } from "@pare/ui";
import { api } from "../lib/ipc";
import { ago, comesBackLabel, date, formatBytes, formatCount, protectExplain, tierBadge } from "../lib/format";
import type { Finding } from "../lib/types";
import { useApp } from "../state/app";
import { categoryHue, findingIcon } from "../components/icons";

function tildePath(p: string, home: string | undefined): string {
  if (home && p.toLowerCase().startsWith(home.toLowerCase())) return `~${p.slice(home.length)}`;
  return p;
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
  const b = tierBadge(f.tier, f.badge);
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
  const location = f.disposal.kind === "empty-recycle-bin" ? f.disposal.drive : (f.paths[0] ?? "");

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
          {formatBytes(f.bytes)}
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

      {f.disposal.kind === "guide" && (
        <section>
          <h3 className="text-body-m-strong text-text-primary">How to clean it up</h3>
          <ol className="mt-1 list-decimal space-y-1 pl-5 text-body-s text-text-secondary">
            {f.disposal.steps.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ol>
          <p className="mt-2 text-caption text-text-tertiary">Pare never deletes this itself.</p>
        </section>
      )}
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
