import type { HTMLAttributes } from "react";
import { Minus, X } from "lucide-react";
import { cx } from "./cx";
import { PareMark } from "./PareMark";

export interface TitleBarProps extends Omit<HTMLAttributes<HTMLDivElement>, "children" | "title"> {
  title?: string;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  /** Swaps the maximize glyph for "restore" and its label. */
  maximized?: boolean;
}

const caption =
  "inline-flex h-10 w-[46px] items-center justify-center text-text-secondary " +
  "transition-[color,background-color,border-color] duration-[var(--pare-motion-fast)] ease-pare " +
  // Inset ring: the buttons sit flush against the window edge.
  "focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-solid focus-visible:outline-accent-default";

/**
 * Custom Windows 11 title bar (Figma `Window/TitleBar` 9-117) for Tauri `decorations: false`.
 * Caption buttons are 46 × 40. Everything left of them carries `data-tauri-drag-region`.
 */
export function TitleBar({
  title = "Pare",
  onMinimize,
  onMaximize,
  onClose,
  maximized = false,
  className,
  ...props
}: TitleBarProps) {
  return (
    <div className={cx("flex h-10 select-none items-center bg-bg-app", className)} {...props}>
      <div
        data-tauri-drag-region=""
        className="flex h-full min-w-0 flex-1 items-center gap-2.5 pl-4"
      >
        <PareMark size={16} />
        <span data-tauri-drag-region="" className="truncate text-caption text-text-secondary">
          {title}
        </span>
      </div>
      <div className="flex h-full">
        <button
          type="button"
          aria-label="Minimize"
          onClick={onMinimize}
          className={cx(caption, "hover:bg-bg-subtle")}
        >
          <Minus aria-hidden="true" size={14} strokeWidth={2} />
        </button>
        <button
          type="button"
          aria-label={maximized ? "Restore" : "Maximize"}
          onClick={onMaximize}
          className={cx(caption, "hover:bg-bg-subtle")}
        >
          {maximized ? (
            <span aria-hidden="true" className="relative size-2.5">
              <span className="absolute -right-0.5 -top-0.5 size-2 rounded-[2px] border border-current" />
              <span className="absolute bottom-0 left-0 size-2 rounded-[2px] border border-current bg-bg-app" />
            </span>
          ) : (
            <span
              aria-hidden="true"
              className="size-2.5 rounded-[2px] border-[1.2px] border-current"
            />
          )}
        </button>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className={cx(caption, "hover:bg-keep-fg hover:text-text-on-accent")}
        >
          <X aria-hidden="true" size={14} strokeWidth={2} />
        </button>
      </div>
    </div>
  );
}
