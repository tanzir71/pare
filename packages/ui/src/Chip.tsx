import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cx, focusRing } from "./cx";

interface ChipOwnProps {
  /** Selected chips use `accent/subtle` and report `aria-pressed="true"`. */
  selected?: boolean;
  /** Optional leading lucide icon (18 px). */
  icon?: LucideIcon;
  children: ReactNode;
  className?: string;
}

export type ChipButtonProps = ChipOwnProps &
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, keyof ChipOwnProps> & { as?: "button" };
/** Non-interactive chip, e.g. a text-only list of tool names. */
export type ChipStaticProps = ChipOwnProps &
  Omit<HTMLAttributes<HTMLSpanElement>, keyof ChipOwnProps> & { as: "span" };
export type ChipProps = ChipButtonProps | ChipStaticProps;

export function chipClassName(selected: boolean, interactive: boolean, className?: string): string {
  return cx(
    "inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border py-2 pl-3.5 pr-4 text-label-m",
    selected
      ? "border-transparent bg-accent-subtle text-accent-on-subtle"
      : "border-border-default bg-bg-surface text-text-primary",
    interactive &&
      cx(
        "cursor-pointer transition-[color,background-color,border-color] duration-[var(--pare-motion-base)] ease-pare",
        !selected && "hover:bg-bg-subtle",
        focusRing,
      ),
    className,
  );
}

/** Scope / filter chip (Figma `Chip` 7-100): Default or Selected, optional icon. */
export function Chip(props: ChipProps) {
  const { selected = false, icon: Icon, children, className, ...rest } = props;
  const content = (
    <>
      {Icon ? <Icon aria-hidden="true" size={18} strokeWidth={2} className="shrink-0" /> : null}
      {children}
    </>
  );

  if (rest.as === "span") {
    const { as: _as, ...spanProps } = rest;
    void _as;
    return (
      <span className={chipClassName(selected, false, className)} {...spanProps}>
        {content}
      </span>
    );
  }
  const { as: _as, type = "button", ...buttonProps } = rest;
  void _as;
  return (
    <button
      type={type}
      aria-pressed={selected}
      className={chipClassName(selected, true, className)}
      {...buttonProps}
    >
      {content}
    </button>
  );
}
