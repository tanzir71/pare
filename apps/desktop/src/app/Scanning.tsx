import { Check, Clock, FolderSearch, RefreshCw, Zap } from "lucide-react";
import { Button, cx } from "@pare/ui";
import { ProgressRing } from "../components/ProgressRing";
import { formatBytes } from "../lib/format";
import { useScan } from "../state/scan";
import { useApp } from "../state/app";

function compact(n: number): string {
  if (n >= 1e6) return `${(n / 1e6).toFixed(2).replace(/\.?0+$/, "")}M`;
  if (n >= 1e4) return `${Math.round(n / 1e3)}K`;
  return n.toLocaleString("en-US");
}

function clock(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function Scanning() {
  const scan = useScan();
  const go = useApp((s) => s.go);
  const p = scan.progress;
  const fraction = p?.fraction ?? 0;
  const phaseNo = !p || p.phase === "enumerating" || p.phase === "starting" ? 1 : 2;
  const pcScope = scan.scope.includes("pc");
  const title = pcScope ? "Scanning This PC" : `Scanning ${scan.scope.length === 1 ? "1 folder" : `${scan.scope.length} folders`}`;
  // Fast mode: whole drives, elevated (the scanner reads the NTFS file table directly).
  const fast = pcScope && (useApp.getState().info?.elevated ?? false);
  const current = p?.current ? p.current.slice(0, 3) : "";

  return (
    <div className="mx-auto flex min-h-full max-w-[560px] flex-col items-center gap-6 px-8 py-10">
      <ProgressRing
        value={fraction}
        label="Scan progress"
        caption={`${current ? `${current.replace(/\\$/, "")} · ` : ""}${phaseNo} of 2 phases`}
      />
      <div className="flex flex-col items-center gap-1.5 text-center">
        <h1 className="text-heading-h2">{title}</h1>
        <p className="flex items-center gap-2 text-body-s text-text-secondary">
          {fast ? (
            <>
              <Zap size={16} className="text-accent-default" aria-hidden="true" />
              Fast mode — reading the NTFS file table directly
            </>
          ) : (
            <>
              <FolderSearch size={16} className="text-accent-default" aria-hidden="true" />
              {phaseNo === 1 ? "Reading your folders. Nothing is changed while Pare scans." : "Checking what's safe to remove"}
            </>
          )}
        </p>
      </div>

      <dl className="grid grid-cols-3 divide-x divide-border-default rounded-lg border border-border-default bg-bg-surface">
        {[
          [compact(p?.files ?? 0), "files read"],
          [formatBytes(p?.foundBytes ?? 0), "found so far"],
          [clock(p?.elapsedMs ?? 0), "elapsed"],
        ].map(([v, l]) => (
          <div key={l} className="flex flex-col items-center px-7 py-3">
            <dd className="tabular text-heading-h3 text-text-primary">{v}</dd>
            <dt className="text-caption text-text-tertiary">{l}</dt>
          </div>
        ))}
      </dl>

      {scan.detectors.length > 0 && (
        <ul className="w-full overflow-hidden rounded-lg border border-border-default bg-bg-surface" aria-label="Checks">
          {scan.detectors.map((d) => (
            <li key={d.id} className="flex items-center gap-3 border-b border-border-default px-4 py-2.5 last:border-b-0">
              <span
                className={cx(
                  "flex size-6 items-center justify-center rounded-full",
                  d.state === "done" && "bg-safe-fg text-text-on-accent",
                  d.state === "running" && "bg-accent-default text-text-on-accent",
                  d.state === "waiting" && "bg-bg-subtle text-text-tertiary",
                )}
                aria-hidden="true"
              >
                {d.state === "done" ? <Check size={14} strokeWidth={3} /> : d.state === "running" ? <RefreshCw size={13} className="pare-spin" /> : <Clock size={14} />}
              </span>
              <span className={cx("flex-1 text-body-m-strong", d.state === "waiting" ? "text-text-tertiary" : "text-text-primary")}>
                {d.label}
              </span>
              <span
                className={cx(
                  "tabular text-body-s",
                  d.state === "running" ? "text-accent-default" : d.state === "waiting" ? "text-text-tertiary" : "text-text-secondary",
                )}
              >
                {d.state === "done" ? formatBytes(d.bytes) : d.state === "running" ? "Checking…" : "Waiting"}
              </span>
            </li>
          ))}
        </ul>
      )}

      <div className="flex gap-3">
        <Button variant="secondary" onClick={() => void scan.cancel()} disabled={!scan.running}>
          Cancel scan
        </Button>
        <Button variant="ghost" onClick={() => go({ name: "clean" })}>
          See results so far
        </Button>
      </div>
      {scan.error && <p className="text-body-s text-keep-fg">{scan.error}</p>}
    </div>
  );
}
