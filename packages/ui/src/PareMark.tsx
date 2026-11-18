import { useId, type SVGProps } from "react";
import { cx } from "./cx";

export interface PareMarkProps extends Omit<SVGProps<SVGSVGElement>, "children"> {
  /** Rendered width and height in px. Minimum 16 (DESIGN.md §0). */
  size?: number;
  /** Accessible name. Without it the mark is decorative (`aria-hidden`). */
  title?: string;
}

/**
 * The approved Pare mark (option E): a storage disk with one slice pared away, on a cobalt
 * gradient tile. Only the top-left corner is curved. Colours come from the brand tokens.
 */
export function PareMark({ size = 32, title, className, ...props }: PareMarkProps) {
  // useId output can contain characters that break `url(#…)`, so keep it to [a-zA-Z0-9_-].
  const gradientId = `pare-mark-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role={title ? "img" : undefined}
      aria-hidden={title ? undefined : true}
      className={cx("shrink-0", className)}
      {...props}
    >
      {title ? <title>{title}</title> : null}
      <defs>
        <linearGradient
          id={gradientId}
          x1="0"
          y1="0"
          x2="64"
          y2="64"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0" style={{ stopColor: "var(--pare-brand-mark-gradient-start)" }} />
          <stop offset="1" style={{ stopColor: "var(--pare-brand-mark-gradient-end)" }} />
        </linearGradient>
      </defs>
      <path d="M30 0H64V64H0V30A30 30 0 0 1 30 0Z" fill={`url(#${gradientId})`} />
      <path d="M30 16A17 17 0 1 0 47 33L30 33Z" style={{ fill: "var(--pare-brand-mark-disk)" }} />
      <path d="M35 11A17 17 0 0 1 52 28L35 28Z" style={{ fill: "var(--pare-brand-mark-slice)" }} />
    </svg>
  );
}
