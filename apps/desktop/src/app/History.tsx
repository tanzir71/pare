import { useCallback, useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, History as HistoryIcon, RotateCcw, Trash2 } from "lucide-react";
import { Badge, Button, cx, focusRing, type BadgeTone } from "@pare/ui";
import { api } from "../lib/ipc";
import { dateTime, formatBytes } from "../lib/format";
import type { HistoryDetail, HistoryOp, ItemStatus } from "../lib/types";
import { t, tp, useT, type MessageKey, type PluralKey } from "../i18n";
import { useApp } from "../state/app";
import { Banner } from "../components/Banner";

const DISPOSAL_LABEL: Record<string, MessageKey> = {
  delete: "history.disposal.delete",
  recycle: "history.disposal.recycle",
  command: "history.disposal.command",
  "empty-recycle-bin": "history.disposal.empty-recycle-bin",
};

const DISPOSAL_COUNT: Record<string, PluralKey> = {
  delete: "history.disposalCount.delete",
  recycle: "history.disposalCount.recycle",
  command: "history.disposalCount.command",
  "empty-recycle-bin": "history.disposalCount.empty-recycle-bin",
};

/** "removed", "to Recycle Bin"…; unknown disposals show as they are. */
function disposalLabel(kind: string): string {
  const key = DISPOSAL_LABEL[kind];
  return key ? t(key) : kind;
}

/** "3 removed", "1 to Recycle Bin"…; unknown disposals show as "n kind". */
function disposalCount(kind: string, n: number): string {
  const key = DISPOSAL_COUNT[kind];
  return key ? tp(key, n) : `${n} ${kind}`;
}

function statusBadge(s: ItemStatus, restored: boolean, purged: boolean): { tone: BadgeTone; label: string } {
  if (restored) return { tone: "accent", label: t("history.status.restored") };
  if (purged) return { tone: "neutral", label: t("history.status.emptied") };
  switch (s) {
    case "ok":
      return { tone: "safe", label: t("history.status.ok") };
    case "partial":
      return { tone: "review", label: t("history.status.partial") };
    case "failed":
      return { tone: "protected", label: t("history.status.failed") };
    case "skipped":
      return { tone: "neutral", label: t("history.status.skipped") };
    default:
      return { tone: "neutral", label: t("history.status.pending") };
  }
}

export function History({ op }: { op?: string }) {
  const { t, tp } = useT();
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
            aria-label={t("history.back")}
            onClick={() => go({ name: "history" })}
            className={cx("flex size-9 items-center justify-center rounded-md border border-border-default bg-bg-surface text-text-secondary hover:bg-bg-subtle", focusRing)}
          >
            <ChevronLeft size={18} aria-hidden="true" />
          </button>
          <div className="mr-auto">
            <h1 className="text-heading-h2">{dateTime(d.op.started)}</h1>
            <p className="tabular text-body-s text-text-secondary">
              {t("history.opSummary", {
                freed: formatBytes(d.op.freedNow),
                recycled: formatBytes(d.op.recycled),
                items: tp("count.items", d.op.items),
              })}
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
                    [
                      tp("history.restoredCount", r.restored),
                      ...(r.collisions.length ? [tp("history.collisions", r.collisions.length)] : []),
                      ...(r.missing.length ? [tp("history.missing", r.missing.length)] : []),
                    ].join(" "),
                  );
                  await refresh();
                }}
              >
                {t("history.restoreAll")}
              </Button>
              <Button
                variant="ghost"
                icon={Trash2}
                onClick={async () => {
                  const r = await api.recyclePurgePareItems(d.op.id);
                  setMessage(t("history.emptiedOp", { items: tp("count.items", r.items), size: formatBytes(r.bytes) }));
                  await refresh();
                }}
              >
                {t("history.emptyThese")}
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
                      <summary className="cursor-pointer text-caption text-text-secondary">{t("history.toolOutput")}</summary>
                      <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-bg-subtle p-2 font-mono text-mono-s text-text-secondary">{it.output}</pre>
                    </details>
                  )}
                </div>
                <span className="text-caption text-text-tertiary">{disposalLabel(it.disposal)}</span>
                <Badge tone={b.tone}>{b.label}</Badge>
                {it.recycled > 0 && !it.restored && !it.purged && (
                  <Button
                    variant="ghost"
                    onClick={async () => {
                      const r = await api.historyRestore(d.op.id, [it.findingId]);
                      setMessage(
                        r.restored
                          ? t("history.restoredOne", { title: it.title })
                          : r.collisions.length
                            ? t("history.collision", { title: it.title, path: r.collisions[0] ?? "" })
                            : t("history.missingOne", { title: it.title }),
                      );
                      await refresh();
                    }}
                  >
                    {t("history.restore")}
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
          <h1 className="text-heading-h1">{t("history.title")}</h1>
          <p className="text-body-s text-text-secondary">{t("history.subtitle")}</p>
        </div>
        {restorableBytes > 0 && (
          <Button
            variant="secondary"
            icon={Trash2}
            onClick={async () => {
              const r = await api.recyclePurgePareItems();
              setMessage(t("history.emptiedAll", { items: tp("count.items", r.items), size: formatBytes(r.bytes) }));
              await refresh();
            }}
          >
            {t("history.emptyPareItems")}
          </Button>
        )}
      </header>
      {message && <Banner>{message}</Banner>}
      {ops && ops.length === 0 && (
        <div className="flex flex-col items-center gap-2 rounded-xl border border-border-default bg-bg-surface px-8 py-14 text-center">
          <HistoryIcon size={28} className="text-text-tertiary" aria-hidden="true" />
          <p className="text-heading-h3">{t("history.empty.title")}</p>
          <p className="text-body-m text-text-secondary">{t("history.empty.body")}</p>
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
                    {tp("count.items", o.items)}
                    {Object.entries(o.disposals).map(([k, n]) => ` · ${disposalCount(k, n)}`)}
                    {o.failed > 0 && ` · ${t("history.failedCount", { n: o.failed })}`}
                  </p>
                </div>
                <div className="tabular text-right">
                  <p className="text-body-m-strong">{t("history.freed", { size: formatBytes(o.freedNow) })}</p>
                  {o.recycled > 0 && <p className="text-caption text-text-tertiary">{t("history.toBin", { size: formatBytes(o.recycled) })}</p>}
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
