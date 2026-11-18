import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cx, focusRing } from "./cx";

interface NavItemOwnProps {
  icon: LucideIcon;
  children: ReactNode;
  /** The current screen: `accent/subtle` fill and `aria-current="page"`. */
  active?: boolean;
  className?: string;
}

export type NavItemProps =
  | (NavItemOwnProps &
      Omit<ButtonHTMLAttributes<HTMLButtonElement>, keyof NavItemOwnProps> & { href?: undefined })
  | (NavItemOwnProps &
      Omit<AnchorHTMLAttributes<HTMLAnchorElement>, keyof NavItemOwnProps> & { href: string });

/** Sidebar navigation item (Figma `NavItem` 7-113): 208 × 40, 20 px icon. `<a>` with `href`, else `<button>`. */
export function NavItem(props: NavItemProps) {
  const { icon: Icon, children, active = false, className, ...rest } = props;
  const classes = cx(
    "flex h-10 w-full items-center gap-3 rounded-md px-3.5 text-left text-label-m no-underline",
    "transition-[color,background-color,border-color] duration-[var(--pare-motion-base)] ease-pare",
    active
      ? "bg-accent-subtle text-accent-on-subtle"
      : "text-text-secondary hover:bg-bg-subtle hover:text-text-primary",
    focusRing,
    className,
  );
  const content = (
    <>
      <Icon aria-hidden="true" size={20} strokeWidth={2} className="shrink-0" />
      <span className="truncate">{children}</span>
    </>
  );

  if (rest.href !== undefined) {
    return (
      <a
        className={classes}
        aria-current={active ? "page" : undefined}
        {...(rest as AnchorHTMLAttributes<HTMLAnchorElement>)}
      >
        {content}
      </a>
    );
  }
  const { type = "button", ...buttonProps } = rest as ButtonHTMLAttributes<HTMLButtonElement>;
  return (
    <button
      type={type}
      className={classes}
      aria-current={active ? "page" : undefined}
      {...buttonProps}
    >
      {content}
    </button>
  );
}
