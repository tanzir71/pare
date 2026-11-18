import type { HTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { CategoryTile, type CategoryHue } from "./CategoryTile";
import { cx } from "./cx";

export interface FeatureCardProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  icon: LucideIcon;
  hue: CategoryHue;
  title: ReactNode;
  children: ReactNode;
  /** Heading element for the title. Default `h3`. */
  titleAs?: "h2" | "h3" | "h4";
}

/** Landing feature card (Figma `FeatureCard` 9-109): 48 px tinted tile, H3 title, Body/M text. */
export function FeatureCard({
  icon,
  hue,
  title,
  children,
  titleAs: Title = "h3",
  className,
  ...props
}: FeatureCardProps) {
  return (
    <article
      className={cx(
        "flex flex-col gap-4 rounded-xl border border-border-default bg-bg-surface p-7",
        className,
      )}
      {...props}
    >
      <CategoryTile icon={icon} hue={hue} size="lg" />
      <Title className="m-0 text-heading-h3 text-text-primary">{title}</Title>
      <p className="m-0 text-body-m text-text-secondary">{children}</p>
    </article>
  );
}
