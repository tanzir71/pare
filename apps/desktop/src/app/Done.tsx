import { useEffect, useState } from "react";
import { Check, History as HistoryIcon, Trash2 } from "lucide-react";
import { Button, cx } from "@pare/ui";
import { api } from "../lib/ipc";
import { categoryTitle, formatBytes } from "../lib/format";
import { useT } from "../i18n";
import type { CleanItemResult, Drive } from "../lib/types";
import { useApp } from "../state/app";
import { useClean } from "../state/clean";
import { Banner } from "../components/Banner";

function useCountUp(target: number, ms = 900): number {
  const reduce = typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const [v, setV] = useState(reduce ? target : 0);
  useEffect(() => {
    if (reduce) {
      setV(target);
      return;
    }
    const start = performance.now();
    let raf = 0;
    const step = (t: number) => {
      const k = Math.min(1, (t - start) / ms);
      setV(target * (1 - Math.pow(1 - k, 3)));
      if (k < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [target, ms, reduce]);
  return v;
}

function used(drives: Drive[]): { used: number; total: number } {
  const fixed = drives.filter((d) => d.isFixed);
  return { used: fixed.reduce((a, d) => a + d.total - d.free, 0), total: fixed.reduce((a, d) => a + d.total, 0) };
}

export function Done() {
  const { t, tp } = useT();
  const go = useApp((s) => s.go);
  const settings = useApp((s) => s.settings);
  const result = useClean((s) => s.result);
  const run = useClean((s) => s.run);
  const [purged, setPurged] = useState(0);
  const [showFailed, setShowFailed] = useState(false);
  const freed = useCountUp(result?.report.freedNow ?? 0);
  if (!result) return null;
  const { report } = result;
  const before = used(result.drivesBefore);
  const after = used(result.drivesAfter);
  const recycled = Math.max(0, report.recycled - purged);
  const failed: CleanItemResult[] = report.results.filter(
    (r) => r.status === "failed" || r.status === "partial" || (r.status === "skipped" && r.reason !== "not-found"),
  );
  const days = settings?.purgePareItemsAfterDays;

  return (
    <div className="mx-auto flex min-h-full max-w-[600px] flex-col items-center gap-6 px-8 py-12 text-center">
      <span className="flex size-[72px] items-center justify-center rounded-full bg-safe-fg text-text-on-accent shadow-[0_12px_32px_color-mix(in_srgb,var(--pare-safe-fg)_35%,transparent)]">
        <Check size={34} strokeWidth={2.5} aria-hidden="true" />
      </span>
      <div>
        <h1 className="tabular text-display-l text-text-primary" aria-live="polite">
          {report.freedNow > 0
            ? t("done.freed", { size: formatBytes(freed) })
            : recycled > 0
              ? t("done.movedToBin")
              : t("done.done")}
        </h1>
        <p className="mt-3 text-body-l text-text-secondary">
          {report.freedNow > 0 && `${t("done.cachesRemoved")} `}
          {recycled > 0
            ? days
              ? t("done.recycledPurge", { size: formatBytes(recycled), days })
              : t("done.recycled", { size: formatBytes(recycled) })
            : report.freedNow > 0
              ? t("done.nothingRecycled")
              : ""}
        </p>
      </div>

      {failed.length > 0 && report.summary && (
        <Banner
          tone="review"
          className="w-full text-left"
          actions={
            <>
              <Button variant="ghost" onClick={() => void run(failed.map((r) => r.findingId))}>
                {t("done.retry")}
              </Button>
              <Button variant="ghost" onClick={() => setShowFailed((v) => !v)} aria-expanded={showFailed}>
                {showFailed ? t("done.hideItems") : t("done.showItems")}
              </Button>
            </>
          }
        >
          {report.summary}
        </Banner>
      )}
      {showFailed && (
        <ul className="w-full overflow-hidden rounded-lg border border-border-default bg-bg-surface text-left">
          {failed.map((r) => (
            <li key={r.findingId} className="border-b border-border-default px-4 py-2 last:border-b-0">
              <p className="truncate text-body-m-strong">{r.title}</p>
              <p className="truncate text-body-s text-text-secondary">{r.error}</p>
            </li>
          ))}
        </ul>
      )}

      <div className="w-full rounded-lg border border-border-default bg-bg-surface px-6 py-4">
        {[
          ["before", t("done.before"), before.used, "bg-border-strong"],
          ["after", t("done.after"), after.used, "bg-accent-default"],
        ].map(([key, label, u, bar]) => (
          <div key={key as string} className="flex items-center gap-4 py-1.5 text-body-m">
            <span className="w-16 text-left text-text-secondary">{label}</span>
            <span className="h-2 flex-1 overflow-hidden rounded-full bg-bg-subtle">
              <span className={cx("block h-full rounded-full", bar as string)} style={{ width: `${before.total ? ((u as number) / before.total) * 100 : 0}%` }} />
            </span>
            <span className="tabular w-28 text-right text-text-primary">{t("done.used", { size: formatBytes(u as number) })}</span>
          </div>
        ))}
      </div>

      {report.byCategory.length > 0 && (
        <dl className="w-full text-body-m">
          {report.byCategory.map((c) => (
            <div key={c.category} className="flex justify-between py-1.5">
              <dt className="text-text-secondary">{categoryTitle(c.category)}</dt>
              <dd className="tabular text-text-primary">
                {c.freed > 0 && formatBytes(c.freed)}
                {c.freed > 0 && c.recycled > 0 && <span className="text-text-secondary"> + </span>}
                {c.recycled > 0 && t("done.inBin", { size: formatBytes(c.recycled) })}
              </dd>
            </div>
          ))}
        </dl>
      )}

      <div className="flex flex-wrap items-center justify-center gap-3">
        <Button variant="ghost" icon={HistoryIcon} onClick={() => go({ name: "history", op: report.opId })}>
          {t("done.viewHistory")}
        </Button>
        {recycled > 0 && (
          <Button
            variant="secondary"
            icon={Trash2}
            onClick={async () => {
              const r = await api.recyclePurgePareItems(report.opId);
              setPurged((p) => p + r.bytes);
            }}
          >
            {t("common.emptyNow", { size: formatBytes(recycled) })}
          </Button>
        )}
        <Button onClick={() => go({ name: "clean" })}>{t("done.done")}</Button>
      </div>
      <p className="text-caption text-text-tertiary">{tp("done.footer", report.ok)}</p>
    </div>
  );
}
