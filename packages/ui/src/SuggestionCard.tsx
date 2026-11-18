import type { HTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Badge, type BadgeTone } from "./Badge";
import { Button } from "./Button";
import { CategoryTile, type CategoryHue } from "./CategoryTile";
import { cx } from "./cx";
import { formatBytes, formatBytesExact } from "./format";

export interface SuggestionCardProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  icon: LucideIcon;
  hue: CategoryHue;
  title: ReactNode;
  description: ReactNode;
  /** Reclaimable bytes (formatted, exact bytes in the tooltip) or a preformatted string. */
  size: number | string;
  /** Caption under the size, e.g. "9.4 GB safe · 3.4 GB to review". */
  meta?: ReactNode;
  /** Dominant tier ("Mostly safe" when mixed). */
  badge: { tone: BadgeTone; label: ReactNode };
  ctaLabel?: string;
  /** Renders the CTA as a link. */
  ctaHref?: string;
  onCtaClick?: () => void;
  /** Heading element for the title. Default `h3`. */
  titleAs?: "h2" | "h3" | "h4" | "p";
  /** Byte formatter for a numeric `size`. Default `formatBytes` ("12.8 GB"). */
  formatSize?: (bytes: number) => string;
  /** Byte formatter for the exact-size tooltip. Default `formatBytesExact`. */
  formatSizeExact?: (bytes: number) => string;
}

/** Clean-tab suggestion card (Figma `SuggestionCard` 9-46). */
export function SuggestionCard({
  icon,
  hue,
  title,
  description,
  size,
  meta,
  badge,
  ctaLabel = "Review",
  ctaHref,
  onCtaClick,
  titleAs: Title = "h3",
  formatSize = formatBytes,
  formatSizeExact = formatBytesExact,
  className,
  ...props
}: SuggestionCardProps) {
  const sizeText = typeof size === "number" ? formatSize(size) : size;
  const sizeTitle = typeof size === "number" ? formatSizeExact(size) : undefined;
  return (
    <article
      className={cx(
        "flex min-w-0 flex-col gap-3.5 rounded-lg border border-border-default bg-bg-surface p-5",
        className,
      )}
      {...props}
    >
      <div className="flex items-center justify-between gap-3">
        <CategoryTile icon={icon} hue={hue} size="md" />
        <Badge tone={badge.tone}>{badge.label}</Badge>
      </div>
      <div className="flex flex-col gap-1">
        <Title className="m-0 text-title-m text-text-primary">{title}</Title>
        <p className="m-0 text-body-s text-text-secondary">{description}</p>
      </div>
      <div className="mt-auto flex items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <span className="text-heading-h2 tabular-nums text-text-primary" title={sizeTitle}>
            {sizeText}
          </span>
          {meta ? <span className="truncate text-caption text-text-tertiary">{meta}</span> : null}
        </div>
        {ctaHref !== undefined ? (
          <Button variant="tonal" href={ctaHref}>
            {ctaLabel}
          </Button>
        ) : (
          <Button variant="tonal" onClick={onCtaClick}>
            {ctaLabel}
          </Button>
        )}
      </div>
    </article>
  );
}
