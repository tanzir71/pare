import type { ReactNode } from "react";
import { AlertTriangle, Info } from "lucide-react";
import { cx } from "@pare/ui";

interface BannerProps {
  tone?: "info" | "review";
  children: ReactNode;
  actions?: ReactNode;
  className?: string;
}

export function Banner({ tone = "info", children, actions, className }: BannerProps) {
  const Icon = tone === "info" ? Info : AlertTriangle;
  return (
    <div
      role={tone === "review" ? "status" : undefined}
      className={cx(
        "flex items-start gap-3 rounded-lg px-4 py-3 text-body-s",
        tone === "info" ? "bg-accent-subtle text-accent-on-subtle" : "bg-review-bg text-review-fg",
        className,
      )}
    >
      <Icon size={18} className="mt-px shrink-0" aria-hidden="true" />
      <div className="min-w-0 flex-1">{children}</div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}
