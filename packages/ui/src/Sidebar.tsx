import type { HTMLAttributes } from "react";
import {
  GitBranch,
  HardDrive,
  History,
  LayoutGrid,
  SlidersHorizontal,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import { cx } from "./cx";
import { formatBytes } from "./format";
import { Lockup } from "./Lockup";
import { NavItem } from "./NavItem";

export type SidebarScreen = "clean" | "browse" | "projects" | "history" | "settings";

const MAIN_ITEMS: ReadonlyArray<{ id: SidebarScreen; label: string; icon: LucideIcon }> = [
  { id: "clean", label: "Clean", icon: Sparkles },
  { id: "browse", label: "Browse", icon: LayoutGrid },
  { id: "projects", label: "Projects", icon: GitBranch },
  { id: "history", label: "History", icon: History },
];

export interface SidebarDrive {
  /** e.g. "Windows (C:)" */
  label: string;
  usedBytes: number;
  totalBytes: number;
  /** Caption under the meter. Default "{used} of {total} used". */
  caption?: string;
  /** Accessible name of the meter. Default "{label} used space". */
  meterLabel?: string;
}

export interface SidebarProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  active?: SidebarScreen;
  /** Drive meter pinned to the bottom. Omit to hide it. */
  drive?: SidebarDrive;
  /** Called with the screen id when a nav item is pressed (button mode). */
  onNavigate?: (screen: SidebarScreen) => void;
  /** Render nav items as links instead of buttons. */
  hrefs?: Partial<Record<SidebarScreen, string>>;
  /** Nav item labels. Defaults: "Clean", "Browse", "Projects", "History", "Settings". */
  labels?: Partial<Record<SidebarScreen, string>>;
  /** Accessible name of the main nav. Default "Main". */
  navLabel?: string;
}

/** App sidebar (Figma `Sidebar` 9-134): 240 wide, lockup, main nav, drive meter and Settings at the bottom. */
export function Sidebar({
  active = "clean",
  drive,
  onNavigate,
  hrefs,
  labels,
  navLabel = "Main",
  className,
  ...props
}: SidebarProps) {
  const item = (id: SidebarScreen, fallback: string, icon: LucideIcon) => {
    const label = labels?.[id] ?? fallback;
    const href = hrefs?.[id];
    return href !== undefined ? (
      <NavItem icon={icon} active={active === id} href={href}>
        {label}
      </NavItem>
    ) : (
      <NavItem
        icon={icon}
        active={active === id}
        onClick={onNavigate ? () => onNavigate(id) : undefined}
      >
        {label}
      </NavItem>
    );
  };
  const usedPct =
    drive && drive.totalBytes > 0 ? Math.min(100, (drive.usedBytes / drive.totalBytes) * 100) : 0;

  return (
    <aside
      className={cx("flex w-60 shrink-0 flex-col bg-bg-app px-4 pb-5 pt-2", className)}
      {...props}
    >
      <div className="pb-5 pl-2.5 pt-2">
        <Lockup markSize={32} />
      </div>
      <nav aria-label={navLabel} className="flex flex-col gap-1">
        {MAIN_ITEMS.map(({ id, label, icon }) => (
          <div key={id}>{item(id, label, icon)}</div>
        ))}
      </nav>
      <div className="min-h-4 flex-1" />
      {drive ? (
        <div className="mb-2 flex flex-col gap-2 rounded-lg border border-border-default bg-bg-surface p-3.5">
          <div className="flex items-center gap-2 text-text-primary">
            <HardDrive aria-hidden="true" size={16} strokeWidth={2} className="shrink-0" />
            <span className="truncate text-label-s">{drive.label}</span>
          </div>
          <div
            role="meter"
            aria-label={drive.meterLabel ?? `${drive.label} used space`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(usedPct)}
            className="h-1.5 overflow-hidden rounded-full bg-bg-subtle"
          >
            <div className="h-full bg-accent-default" style={{ width: `${usedPct}%` }} />
          </div>
          <span className="text-caption text-text-secondary">
            {drive.caption ?? `${formatBytes(drive.usedBytes)} of ${formatBytes(drive.totalBytes)} used`}
          </span>
        </div>
      ) : null}
      {item("settings", "Settings", SlidersHorizontal)}
    </aside>
  );
}
