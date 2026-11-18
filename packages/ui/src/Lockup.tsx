import type { HTMLAttributes } from "react";
import { cx } from "./cx";
import { PareMark } from "./PareMark";

export interface LockupProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  /** Mark size in px. The wordmark (Geist SemiBold, −3%) is 75% of it; the gap is ~31%. Default 32. */
  markSize?: number;
}

/** Logo lockup: mark + "Pare" wordmark (DESIGN.md §0, Figma `brand/lockup` 6-206). */
export function Lockup({ markSize = 32, className, style, ...props }: LockupProps) {
  return (
    <span
      className={cx("inline-flex items-center font-semibold text-text-primary", className)}
      style={{ gap: Math.round(markSize * 0.3125), ...style }}
      {...props}
    >
      <PareMark size={markSize} />
      <span style={{ fontSize: markSize * 0.75, lineHeight: 1, letterSpacing: "-0.03em" }}>
        Pare
      </span>
    </span>
  );
}
