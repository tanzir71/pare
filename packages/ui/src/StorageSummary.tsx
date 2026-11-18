import type { HTMLAttributes, ReactNode } from "react";
import { WandSparkles } from "lucide-react";
import { Button } from "./Button";
import { cx } from "./cx";
import { formatBytes, formatBytesExact } from "./format";

export interface StorageSummaryProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  /** Eyebrow, e.g. "Scan complete · 2 min ago · Drives C: and D:" (rendered uppercase). */
  overline?: ReactNode;
  safeBytes: number;
  reviewBytes: number;
  otherUsedBytes: number;
  freeBytes: number;
  /** Default: "Free up {safe}". The primary action only ever frees Safe items. */
  primaryLabel?: string;
  caption?: ReactNode;
  secondaryLabel?: string;
  onFreeUp?: () => void;
  onReviewAll?: () => void;
  /** Heading element for the big number. Default `h2`. */
  headingAs?: "h1" | "h2" | "h3" | "p";
  /** Text after the big number. Default "can be freed". */
  reclaimLabel?: ReactNode;
  /** Legend and bar labels. Defaults: "Safe to free", "To review", "Other used", "Free". */
  segmentLabels?: Partial<Record<SegmentKey, string>>;
  /** Byte formatter for the visible numbers. Default `formatBytes` ("12.8 GB"). */
  formatSize?: (bytes: number) => string;
  /** Byte formatter for the exact-size tooltip. Default `formatBytesExact`. */
  formatSizeExact?: (bytes: number) => string;
}

type SegmentKey = "other" | "safe" | "review" | "free";

const SEGMENTS = [
  { key: "other", label: "Other used", bar: "bg-accent-default" },
  { key: "safe", label: "Safe to free", bar: "bg-safe-fg" },
  { key: "review", label: "To review", bar: "bg-review-fg" },
  { key: "free", label: "Free", bar: "bg-bg-subtle" },
] as const;

/** The legend reads Safe, Review, Other, Free; the bar draws Other, Safe, Review, Free. */
const LEGEND_ORDER = ["safe", "review", "other", "free"] as const;

/**
 * Storage summary card (Clean home, DESIGN.md §3): overline, "40.6 GB can be freed",
 * segmented usage bar with legend, and the Primary L "Free up" action.
 * It is a container: under ~720 px wide the action column stacks below the summary.
 */
export function StorageSummary({
  overline,
  safeBytes,
  reviewBytes,
  otherUsedBytes,
  freeBytes,
  primaryLabel,
  caption = "Safe items only · your own files stay restorable",
  secondaryLabel = "Review everything first",
  onFreeUp,
  onReviewAll,
  headingAs: Heading = "h2",
  reclaimLabel = "can be freed",
  segmentLabels,
  formatSize = formatBytes,
  formatSizeExact = formatBytesExact,
  className,
  ...props
}: StorageSummaryProps) {
  const bytes = { safe: safeBytes, review: reviewBytes, other: otherUsedBytes, free: freeBytes };
  const total = safeBytes + reviewBytes + otherUsedBytes + freeBytes;
  const reclaimable = safeBytes + reviewBytes;
  const labelFor = (key: SegmentKey) => {
    const s = SEGMENTS.find((x) => x.key === key)!;
    return { ...s, label: segmentLabels?.[key] ?? s.label };
  };
  const barLabel = LEGEND_ORDER.map((k) => `${labelFor(k).label} ${formatSize(bytes[k])}`).join(
    ", ",
  );

  return (
    <section
      className={cx("@container rounded-xl border border-border-default bg-bg-surface", className)}
      {...props}
    >
      <div className="flex flex-col gap-6 p-5 @min-[720px]:flex-row @min-[720px]:items-center @min-[720px]:gap-8 @min-[720px]:p-7">
        <div className="flex min-w-0 flex-1 flex-col gap-3">
          {overline ? (
            <p className="m-0 text-overline uppercase text-text-tertiary">{overline}</p>
          ) : null}
          <Heading className="m-0 flex flex-wrap items-baseline gap-x-2.5 font-semibold">
            <span
              className="text-numeric-xl tabular-nums text-text-primary @min-[720px]:text-display-l"
              title={formatSizeExact(reclaimable)}
            >
              {formatSize(reclaimable)}
            </span>
            <span className="text-heading-h3 text-text-secondary">{reclaimLabel}</span>
          </Heading>
          {total > 0 ? (
            <div
              role="img"
              aria-label={barLabel}
              className="flex h-3 w-full max-w-[640px] gap-0.5 overflow-hidden rounded-full"
            >
              {SEGMENTS.map((s) =>
                bytes[s.key] > 0 ? (
                  <span
                    key={s.key}
                    className={cx("h-full min-w-1", s.bar)}
                    style={{ flex: `${((bytes[s.key] / total) * 1000).toFixed(3)} 1 0` }}
                  />
                ) : null,
              )}
            </div>
          ) : null}
          <ul aria-hidden="true" className="m-0 flex list-none flex-wrap gap-x-5 gap-y-1 p-0">
            {LEGEND_ORDER.map((k) => (
              <li key={k} className="flex items-center gap-1.5 text-body-s text-text-secondary">
                <span
                  className={cx(
                    "size-2 shrink-0 rounded-full",
                    k === "free" ? "bg-border-strong" : labelFor(k).bar,
                  )}
                />
                <span className="tabular-nums">
                  {labelFor(k).label} · {formatSize(bytes[k])}
                </span>
              </li>
            ))}
          </ul>
        </div>
        <div className="flex shrink-0 flex-col items-stretch gap-2 text-center @min-[720px]:items-center">
          <Button
            variant="primary"
            size="l"
            icon={WandSparkles}
            onClick={onFreeUp}
            disabled={safeBytes <= 0}
          >
            {primaryLabel ?? `Free up ${formatSize(safeBytes)}`}
          </Button>
          {caption ? <p className="m-0 text-caption text-text-tertiary">{caption}</p> : null}
          <Button variant="ghost" onClick={onReviewAll}>
            {secondaryLabel}
          </Button>
        </div>
      </div>
    </section>
  );
}
