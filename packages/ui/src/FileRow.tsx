import type { HTMLAttributes, ReactNode } from "react";
import { ChevronRight, type LucideIcon } from "lucide-react";
import { Badge, type BadgeTone } from "./Badge";
import { CategoryTile, type CategoryHue } from "./CategoryTile";
import { Checkbox } from "./Checkbox";
import { cx } from "./cx";
import { formatBytes, formatBytesExact } from "./format";

export interface FileRowProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  icon: LucideIcon;
  hue: CategoryHue;
  name: string;
  /** Full path, shown in Geist Mono and truncated to one line. */
  path: string;
  badge: { tone: BadgeTone; label: ReactNode };
  /** Relative age or reason, e.g. "Opened 8 months ago". */
  age?: ReactNode;
  /** Bytes (formatted, exact in the tooltip) or a preformatted string. */
  size: number | string;
  checked?: boolean;
  /** Protected rows: the checkbox is disabled and can never be selected. */
  disabled?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  /** Hide the trailing chevron (no details panel). */
  hideChevron?: boolean;
  /** Accessible name of the checkbox. Default "Select {name}". */
  selectLabel?: string;
  /** Byte formatter for a numeric `size`. Default `formatBytes` ("12.8 GB"). */
  formatSize?: (bytes: number) => string;
  /** Byte formatter for the exact-size tooltip. Default `formatBytesExact`. */
  formatSizeExact?: (bytes: number) => string;
}

/**
 * Review-list row (Figma `FileRow` 9-68): checkbox, tile, name + mono path, badge, age, size.
 * The row is a size container: under 560 px wide the badge moves under the path and age/chevron hide.
 */
export function FileRow({
  icon,
  hue,
  name,
  path,
  badge,
  age,
  size,
  checked = false,
  disabled = false,
  onCheckedChange,
  hideChevron = false,
  selectLabel,
  formatSize = formatBytes,
  formatSizeExact = formatBytesExact,
  className,
  ...props
}: FileRowProps) {
  const sizeText = typeof size === "number" ? formatSize(size) : size;
  const sizeTitle = typeof size === "number" ? formatSizeExact(size) : undefined;
  return (
    <div
      className={cx("@container border-b border-border-default bg-bg-surface", className)}
      {...props}
    >
      <div className="flex min-w-0 items-center gap-2.5 px-4 py-2.5">
        <Checkbox
          aria-label={selectLabel ?? `Select ${name}`}
          checked={checked}
          disabled={disabled}
          readOnly={onCheckedChange === undefined}
          onChange={onCheckedChange ? (e) => onCheckedChange(e.currentTarget.checked) : undefined}
        />
        <CategoryTile icon={icon} hue={hue} size="sm" />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span data-slot="name" className="truncate text-body-m-strong text-text-primary">
            {name}
          </span>
          <span
            data-slot="path"
            className="truncate font-mono text-mono-s text-text-tertiary"
            title={path}
          >
            {path}
          </span>
          {/* Narrow rows (under 560 px) move the badge under the path and drop age and chevron. */}
          <Badge tone={badge.tone} className="mt-1 self-start @min-[560px]:hidden">
            {badge.label}
          </Badge>
        </div>
        <Badge tone={badge.tone} className="@max-[559.98px]:hidden">
          {badge.label}
        </Badge>
        {age !== undefined ? (
          <span
            data-slot="age"
            className="w-[116px] shrink-0 truncate text-body-s text-text-secondary @max-[559.98px]:hidden"
          >
            {age}
          </span>
        ) : null}
        <span
          data-slot="size"
          className="w-16 shrink-0 text-right text-body-m-strong tabular-nums text-text-primary"
          title={sizeTitle}
        >
          {sizeText}
        </span>
        {hideChevron ? null : (
          <ChevronRight
            aria-hidden="true"
            size={18}
            strokeWidth={2}
            className="shrink-0 text-text-tertiary @max-[559.98px]:hidden"
          />
        )}
      </div>
    </div>
  );
}
