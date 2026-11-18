import { useCallback, useEffect, useRef, type InputHTMLAttributes, type Ref } from "react";
import { Check, Minus } from "lucide-react";
import { cx, focusRing } from "./cx";

export interface CheckboxProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "type" | "size"
> {
  /** Tri-state "mixed" (group header with some rows selected). Set as a DOM property, so it needs JS. */
  indeterminate?: boolean;
  ref?: Ref<HTMLInputElement>;
  /** Class for the 20 px wrapper. */
  className?: string;
}

/**
 * Checkbox (Figma 7-91): a real `<input type="checkbox">` drawn as a 20 px box.
 * Unchecked, Checked, Indeterminate, Disabled (= protected item: subtle fill, never selectable).
 * Give it an accessible name with `aria-label` or a wrapping/linked `<label>`.
 */
export function Checkbox({ indeterminate = false, className, ref, ...props }: CheckboxProps) {
  const inner = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (inner.current) inner.current.indeterminate = indeterminate;
  }, [indeterminate]);

  const setRef = useCallback(
    (el: HTMLInputElement | null) => {
      inner.current = el;
      if (typeof ref === "function") ref(el);
      else if (ref) ref.current = el;
    },
    [ref],
  );

  return (
    <span className={cx("relative inline-flex size-5 shrink-0", className)}>
      <input
        ref={setRef}
        type="checkbox"
        className={cx(
          "peer size-5 cursor-pointer appearance-none rounded-sm border-[1.5px] border-border-strong bg-bg-surface",
          "transition-[color,background-color,border-color] duration-[var(--pare-motion-fast)] ease-pare hover:border-accent-default",
          "checked:border-accent-default checked:bg-accent-default",
          "indeterminate:border-accent-default indeterminate:bg-accent-default",
          "disabled:cursor-not-allowed disabled:border-border-default disabled:bg-bg-subtle",
          focusRing,
        )}
        {...props}
      />
      <Check
        aria-hidden="true"
        size={14}
        strokeWidth={3}
        className="pointer-events-none absolute inset-0 m-auto hidden text-text-on-accent peer-checked:block peer-indeterminate:hidden peer-disabled:hidden"
      />
      <Minus
        aria-hidden="true"
        size={14}
        strokeWidth={3}
        className="pointer-events-none absolute inset-0 m-auto hidden text-text-on-accent peer-indeterminate:block peer-disabled:hidden"
      />
    </span>
  );
}
