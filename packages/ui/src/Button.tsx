import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cx, focusRing } from "./cx";

export type ButtonVariant = "primary" | "tonal" | "secondary" | "ghost" | "danger";
/** M = 40 px (app), L = 48 px (landing and the one big action per screen). */
export type ButtonSize = "m" | "l";

interface ButtonOwnProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Optional leading lucide icon (18 px at M, 20 px at L). */
  icon?: LucideIcon;
  /** Stretch to the container width. */
  fullWidth?: boolean;
  children: ReactNode;
  className?: string;
}

export type ButtonAsButtonProps = ButtonOwnProps &
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, keyof ButtonOwnProps> & { href?: undefined };
export type ButtonAsLinkProps = ButtonOwnProps &
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, keyof ButtonOwnProps> & { href: string };
/** Renders an `<a>` when `href` is given, otherwise a `<button type="button">`. */
export type ButtonProps = ButtonAsButtonProps | ButtonAsLinkProps;

const base =
  "inline-flex shrink-0 items-center justify-center whitespace-nowrap rounded-md select-none no-underline " +
  "transition-[background-color,color,border-color,filter] duration-[var(--pare-motion-base)] ease-pare " +
  "active:brightness-[0.98] aria-disabled:pointer-events-none aria-disabled:opacity-40 " +
  "disabled:cursor-not-allowed disabled:opacity-40";

const sizes: Record<ButtonSize, string> = {
  m: "h-10 gap-2 px-4 text-label-m",
  l: "h-12 gap-2.5 px-[22px] text-title-m",
};

const variants: Record<ButtonVariant, string> = {
  primary: "bg-accent-default text-text-on-accent hover:bg-accent-hover",
  tonal:
    "bg-accent-subtle text-accent-on-subtle " +
    "hover:bg-[color-mix(in_srgb,var(--pare-accent-subtle),var(--pare-accent-default)_8%)]",
  secondary: "border border-border-strong bg-bg-surface text-text-primary hover:bg-bg-subtle",
  ghost: "bg-transparent text-accent-default hover:bg-accent-subtle",
  danger: "bg-keep-fg text-text-on-accent hover:brightness-90",
};

export function buttonClassName({
  variant = "primary",
  size = "m",
  fullWidth = false,
  className,
}: Pick<ButtonOwnProps, "variant" | "size" | "fullWidth" | "className">): string {
  return cx(base, focusRing, sizes[size], variants[variant], fullWidth && "w-full", className);
}

/** Button (Figma `Button` 7-66): Primary, Tonal, Secondary, Ghost, Danger × M / L. */
export function Button(props: ButtonProps) {
  const { variant, size = "m", icon: Icon, fullWidth, children, className, ...rest } = props;
  const classes = buttonClassName({ variant, size, fullWidth, className });
  const content = (
    <>
      {Icon ? (
        <Icon
          aria-hidden="true"
          size={size === "l" ? 20 : 18}
          strokeWidth={2}
          className="shrink-0"
        />
      ) : null}
      <span>{children}</span>
    </>
  );

  if (rest.href !== undefined) {
    const anchorProps = rest as Omit<ButtonAsLinkProps, keyof ButtonOwnProps>;
    return (
      <a className={classes} {...anchorProps}>
        {content}
      </a>
    );
  }
  const { type = "button", ...buttonProps } = rest as Omit<
    ButtonAsButtonProps,
    keyof ButtonOwnProps
  >;
  return (
    <button type={type} className={classes} {...buttonProps}>
      {content}
    </button>
  );
}
