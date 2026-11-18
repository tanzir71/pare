import type { HTMLAttributes } from "react";
import type { LucideIcon } from "lucide-react";
import { cx } from "./cx";

/**
 * Category hues (DESIGN.md §1): violet = AI tools, orange = projects, teal = dev caches,
 * pink = large files, blue = downloads/screenshots, slate = Recycle Bin/protected,
 * amber = duplicates, green = junk.
 */
export type CategoryHue =
  "violet" | "blue" | "teal" | "orange" | "pink" | "green" | "amber" | "slate";

/** `bg` + `fg` token classes for each hue. Text on the tile always uses the paired fg token. */
export const hueClasses: Record<CategoryHue, string> = {
  violet: "bg-cat-violet-bg text-cat-violet-fg",
  blue: "bg-cat-blue-bg text-cat-blue-fg",
  teal: "bg-cat-teal-bg text-cat-teal-fg",
  orange: "bg-cat-orange-bg text-cat-orange-fg",
  pink: "bg-cat-pink-bg text-cat-pink-fg",
  green: "bg-cat-green-bg text-cat-green-fg",
  amber: "bg-cat-amber-bg text-cat-amber-fg",
  slate: "bg-cat-slate-bg text-cat-slate-fg",
};

/** sm = 36 (rows), md = 44 (suggestion cards), lg = 48 (landing feature cards). */
export type CategoryTileSize = "sm" | "md" | "lg";

const sizes: Record<CategoryTileSize, { box: string; icon: number }> = {
  sm: { box: "size-9 rounded-md", icon: 18 },
  md: { box: "size-11 rounded-md", icon: 22 },
  lg: { box: "size-12 rounded-[10px]", icon: 24 },
};

export interface CategoryTileProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  icon: LucideIcon;
  hue: CategoryHue;
  size?: CategoryTileSize;
}

/** Square icon tile tinted with a category hue. Decorative: the label sits next to it. */
export function CategoryTile({
  icon: Icon,
  hue,
  size = "md",
  className,
  ...props
}: CategoryTileProps) {
  const s = sizes[size];
  return (
    <span
      aria-hidden="true"
      className={cx(
        "inline-flex shrink-0 items-center justify-center",
        s.box,
        hueClasses[hue],
        className,
      )}
      {...props}
    >
      <Icon size={s.icon} strokeWidth={2} />
    </span>
  );
}
