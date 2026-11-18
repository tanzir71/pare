import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { cx, focusRing } from "@pare/ui";
import { useT } from "../i18n";

interface SheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  /** "center" dialog (default) or a right "side" sheet. */
  placement?: "center" | "side";
  className?: string;
}

/** Modal sheet on the native <dialog> element: focus is trapped, Esc closes. */
export function Sheet({ open, onClose, title, description, children, footer, placement = "center", className }: SheetProps) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      aria-labelledby="sheet-title"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
      className={cx(
        "m-auto max-h-[85vh] overflow-hidden border border-border-default bg-bg-surface p-0 text-text-primary shadow-floating",
        placement === "side"
          ? "mr-0 h-full max-h-full w-[360px] rounded-none rounded-l-lg"
          : "w-[min(520px,calc(100vw-48px))] rounded-xl",
        className,
      )}
    >
      {open && (
        <div className="flex max-h-[inherit] flex-col">
          <div className="flex items-start justify-between gap-4 px-6 pb-2 pt-5">
            <div className="min-w-0">
              <h2 id="sheet-title" className="text-heading-h3 text-text-primary">
                {title}
              </h2>
              {description && <p className="mt-1 text-body-s text-text-secondary">{description}</p>}
            </div>
            <button
              type="button"
              aria-label={t("common.close")}
              onClick={onClose}
              className={cx("-mr-2 rounded-md p-2 text-text-secondary hover:bg-bg-subtle", focusRing)}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-auto px-6 py-3">{children}</div>
          {footer && <div className="flex justify-end gap-3 border-t border-border-default px-6 py-4">{footer}</div>}
        </div>
      )}
    </dialog>
  );
}
