import { useCallback, useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, History as HistoryIcon, RotateCcw, Trash2 } from "lucide-react";
import { Badge, Button, cx, focusRing, type BadgeTone } from "@pare/ui";
import { api } from "../lib/ipc";
import { dateTime, formatBytes, plural } from "../lib/format";
import type { HistoryDetail, HistoryOp, ItemStatus } from "../lib/types";
import { useApp } from "../state/app";
import { Banner } from "../components/Banner";

const DISPOSAL_LABEL: Record<string, string> = {
  delete: "removed",
  recycle: "to Recycle Bin",
  command: "tool command",
  "empty-recycle-bin": "bin emptied",
};

function statusBadge(s: ItemStatus, restored: boolean, purged: boolean): { tone: BadgeTone; label: string } {
  if (restored) return { tone: "accent", label: "Restored" };
  if (purged) return { tone: "neutral", label: "Emptied" };
  switch (s) {
    case "ok":
      return { tone: "safe", label: "Done" };
    case "partial":
      return { tone: "review", label: "Partly done" };
    case "failed":
      return { tone: "protected", label: "Failed" };
    case "skipped":
      return { tone: "neutral", label: "Skipped" };
    default:
      return { tone: "neutral", label: "Pending" };
  }
}

export function History({ op }: { op?: string }) {
  const go = useApp((s) => s.go);
  const [ops, setOps] = useState<HistoryOp[] | null>(null);
  const [detail, setDetail] = useState<HistoryDetail | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setOps(await api.historyList());
    if (op) setDetail(await api.historyGet(op));
    else setDetail(null);
  }, [op]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const restorableBytes = (ops ?? []).filter((o) => o.restorable > 0).reduce((a, o) => a + o.recycled, 0);

  if (op && detail) {
    const d = detail;
    return (
      <div className="flex flex-col gap-5 pb-10 pl-4 pr-8 pt-6">
        <header className="flex items-center gap-3">
          <button
            type="button"
            aria-label="Back to History"
            onClick={() => go({ name: "history" })}
            className={cx("flex size-9 items-center justify-center rounded-md border border-border-default bg-bg-surface text-text-secondary hover:bg-bg-subtle", focusRing)}
          >
            <ChevronLeft size={18} aria-hidden="true" />
          </button>
          <div className="mr-auto">
            <h1 className="text-heading-h2">{dateTime(d.op.started)}</h1>
            <p className="tabular text-body-s text-text-secondary">
              {formatBytes(d.op.freedNow)} freed · {formatBytes(d.op.recycled)} to Recycle Bin · {plural(d.op.items, "item")}
            </p>
          </div>
          {d.op.restorable > 0 && (
            <>
              <Button
                variant="secondary"
                icon={RotateCcw}
                onClick={async () => {
                  const r = await api.historyRestore(d.op.id);
                  setMessage(
                    `${plural(r.restored, "item")} restored.` +
                      (r.collisions.length ? ` ${plural(r.collisions.length, "item")} kept in the Recycle Bin because something is already at the original location.` : "") +
                      (r.missing.length ? ` ${plural(r.missing.length, "item")} are no longer in the Recycle Bin.` : ""),
                  );
                  await refresh();
                }}
              >
                Restore all
              </Button>
              <Button
                variant="ghost"
                icon={Trash2}
                onClick={async () => {
                  const r = await api.recyclePurgePareItems(d.op.id);
                  setMessage(`Emptied ${plural(r.items, "item")} (${formatBytes(r.bytes)}) from the Recycle Bin.`);
                  await refresh();
                }}
              >
                Empty these now
              </Button>
            </>
          )}
        </header>
        {message && <Banner>{message}</Banner>}
        <ul className="overflow-hidden rounded-lg border border-border-default bg-bg-surface">
          {d.items.map((it) => {
            const b = statusBadge(it.result, it.restored, it.purged);
            return (
              <li key={it.findingId + it.path} className="flex items-center gap-3 border-b border-border-default px-4 py-2.5 last:border-b-0">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-body-m-strong">{it.title}</p>
                  <p className="truncate font-mono text-mono-s text-text-tertiary">{it.path}</p>
                  {it.error && <p className="truncate text-caption text-keep-fg">{it.error}</p>}
                  {it.output && (
                    <details className="mt-1">
                      <summary className="cursor-pointer text-caption text-text-secondary">Tool output</summary>
                      <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-bg-subtle p-2 font-mono text-mono-s text-text-secondary">{it.output}</pre>
                    </details>
                  )}
                </div>
                <span className="text-caption text-text-tertiary">{DISPOSAL_LABEL[it.disposal] ?? it.disposal}</span>
                <Badge tone={b.tone}>{b.label}</Badge>
                {it.recycled > 0 && !it.restored && !it.purged && (
                  <Button
                    variant="ghost"
                    onClick={async () => {
                      const r = await api.historyRestore(d.op.id, [it.findingId]);
                      setMessage(
                        r.restored
                          ? `Restored ${it.title}.`
                          : r.collisions.length
                            ? `${it.title} wasn't restored: something already exists at ${r.collisions[0]}.`
                            : `${it.title} is no longer in the Recycle Bin.`,
                      );
                      await refresh();
                    }}
                  >
                    Restore
                  </Button>
                )}
                <span className="tabular w-20 text-right text-body-m">{formatBytes(it.freed + it.recycled || it.bytes)}</span>
              </li>
            );
          })}
        </ul>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-5 pb-10 pl-4 pr-8 pt-6">
      <header className="flex items-center gap-3">
        <div className="mr-auto">
          <h1 className="text-heading-h1">History</h1>
          <p className="text-body-s text-text-secondary">Every cleanup Pare ran on this PC, newest first.</p>
        </div>
        {restorableBytes > 0 && (
          <Button
            variant="secondary"
            icon={Trash2}
            onClick={async () => {
              const r = await api.recyclePurgePareItems();
              setMessage(`Emptied ${plural(r.items, "item")} (${formatBytes(r.bytes)}) Pare had moved to the Recycle Bin.`);
              await refresh();
            }}
          >
            Empty Pare's items now
          </Button>
        )}
      </header>
      {message && <Banner>{message}</Banner>}
      {ops && ops.length === 0 && (
        <div className="flex flex-col items-center gap-2 rounded-xl border border-border-default bg-bg-surface px-8 py-14 text-center">
          <HistoryIcon size={28} className="text-text-tertiary" aria-hidden="true" />
          <p className="text-heading-h3">No cleanups yet</p>
          <p className="text-body-m text-text-secondary">When you free up space, each run shows up here with exactly what happened.</p>
        </div>
      )}
      {ops && ops.length > 0 && (
        <ul className="overflow-hidden rounded-lg border border-border-default bg-bg-surface">
          {ops.map((o) => (
            <li key={o.id} className="border-b border-border-default last:border-b-0">
              <button
                type="button"
                onClick={() => go({ name: "history", op: o.id })}
                className={cx("flex w-full items-center gap-4 px-4 py-3 text-left hover:bg-bg-subtle", focusRing)}
              >
                <div className="min-w-0 flex-1">
                  <p className="text-body-m-strong">{dateTime(o.started)}</p>
                  <p className="text-body-s text-text-secondary">
                    {plural(o.items, "item")}
                    {Object.entries(o.disposals).map(([k, n]) => ` · ${n} ${DISPOSAL_LABEL[k] ?? k}`)}
                    {o.failed > 0 && ` · ${o.failed} failed`}
                  </p>
                </div>
                <div className="tabular text-right">
                  <p className="text-body-m-strong">{formatBytes(o.freedNow)} freed</p>
                  {o.recycled > 0 && <p className="text-caption text-text-tertiary">{formatBytes(o.recycled)} to Recycle Bin</p>}
                </div>
                <ChevronRight size={18} className="text-text-tertiary" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
