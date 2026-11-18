import type { InputHTMLAttributes } from "react";
import { Search } from "lucide-react";
import { cx } from "./cx";

export interface SearchFieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "type" | "size"
> {
  /** Accessible name. Defaults to the placeholder. */
  label?: string;
  /** Class for the wrapper (width etc.). */
  className?: string;
}

/** Search input with a leading icon (Clean header, 300 wide in the app). */
export function SearchField({
  label,
  placeholder = "Search files and folders",
  className,
  ...props
}: SearchFieldProps) {
  return (
    <label
      className={cx(
        "flex h-10 min-w-0 items-center gap-2.5 rounded-md border border-border-default bg-bg-surface pl-3.5 pr-4",
        "focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-accent-default",
        className,
      )}
    >
      <Search
        aria-hidden="true"
        size={18}
        strokeWidth={2}
        className="shrink-0 text-text-tertiary"
      />
      <input
        type="search"
        aria-label={label ?? placeholder}
        placeholder={placeholder}
        className="min-w-0 flex-1 bg-transparent text-body-m text-text-primary outline-none placeholder:text-text-tertiary"
        {...props}
      />
    </label>
  );
}
