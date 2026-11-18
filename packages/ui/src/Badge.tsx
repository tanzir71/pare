import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "./cx";

export type BadgeTone = "safe" | "review" | "protected" | "neutral" | "accent";

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: BadgeTone;
  children: ReactNode;
  /** Extra text for screen readers, e.g. "rebuilds automatically" → announced "Safe: rebuilds automatically". */
  srDescription?: string;
}

const tones: Record<BadgeTone, string> = {
  safe: "bg-safe-bg text-safe-fg",
  review: "bg-review-bg text-review-fg",
  protected: "bg-keep-bg text-keep-fg",
  neutral: "bg-bg-subtle text-text-secondary",
  accent: "bg-accent-subtle text-accent-on-subtle",
};

/**
 * Safety badge (Figma `Badge` 7-82): a pill with a 6 px dot. Every tier carries a text label,
 * so colour is never the only signal. Protected badges read "Keep", "In use" or "Uncommitted".
 */
export function Badge({
  tone = "neutral",
  children,
  srDescription,
  className,
  ...props
}: BadgeProps) {
  return (
    <span
      className={cx(
        "inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full px-2.5 py-1 text-label-s",
        tones[tone],
        className,
      )}
      {...props}
    >
      <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-current" />
      {children}
      {srDescription ? <span className="sr-only">: {srDescription}</span> : null}
    </span>
  );
}
