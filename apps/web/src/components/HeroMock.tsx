import {
  Button,
  Chip,
  SearchField,
  Sidebar,
  StorageSummary,
  SuggestionCard,
  TitleBar,
  cx,
} from "@pare/ui";
import {
  Download,
  File,
  Folder,
  HardDrive,
  Monitor,
  Package,
  Plus,
  RefreshCw,
  Sparkles,
  Terminal,
} from "lucide-react";

const GB = 1e9;

/**
 * The Clean home screen as a decorative, non-interactive mini window, built from the same
 * @pare/ui components as the app. It is laid out at the app's real size (1280 wide) and scaled
 * with CSS `zoom` from 768 px up (1120 wide at 1440). Below 768 px it shows the window chrome,
 * the storage summary and one suggestion card at natural size (DESIGN.md §6.3).
 * The page wraps it in `aria-hidden` + `inert`, so the buttons inside are never reachable.
 */
export function HeroMock({ className }: { className?: string }) {
  return (
    <div
      className={cx(
        "mx-auto w-full overflow-hidden rounded-lg border border-border-default bg-bg-app shadow-hero",
        "md:w-[1280px] md:[zoom:0.53] lg:[zoom:0.725] xl:[zoom:0.875]",
        className,
      )}
    >
      <TitleBar />
      <div className="flex">
        <Sidebar
          active="clean"
          drive={{ label: "Windows (C:)", usedBytes: 412 * GB, totalBytes: 512 * GB }}
          className="max-md:hidden"
        />
        <div className="flex min-w-0 flex-1 flex-col gap-6 px-4 pb-4 pt-2 md:pb-8 md:pr-8">
          <div className="hidden items-center gap-3 md:flex">
            <p className="m-0 flex-1 text-heading-h1 text-text-primary">Clean</p>
            <SearchField className="w-[300px]" readOnly />
            <Button variant="secondary" icon={RefreshCw}>
              Scan again
            </Button>
          </div>
          <StorageSummary
            headingAs="p"
            overline="Scan complete · 2 min ago · Drives C: and D:"
            safeBytes={28 * GB}
            reviewBytes={12.6 * GB}
            otherUsedBytes={371.4 * GB}
            freeBytes={100 * GB}
          />
          <div className="hidden items-center gap-2 md:flex">
            <span className="text-body-m-strong font-semibold text-text-secondary">Scope</span>
            <Chip selected icon={HardDrive}>
              This PC
            </Chip>
            <Chip icon={Monitor}>Desktop</Chip>
            <Chip icon={Download}>Downloads</Chip>
            <Chip icon={File}>Documents</Chip>
            <Chip icon={Folder}>D:\code</Chip>
            <Chip icon={Plus}>Add folder</Chip>
          </div>
          <div className="hidden items-baseline gap-3 md:flex">
            <p className="m-0 text-heading-h3 text-text-primary">Suggestions</p>
            <p className="m-0 text-body-s text-text-tertiary">
              9 categories · sorted by space you get back
            </p>
          </div>
          <div className="grid gap-4 md:grid-cols-3">
            <SuggestionCard
              titleAs="p"
              icon={Sparkles}
              hue="violet"
              title="AI tool caches"
              description="Claude Code, Codex and Cursor sessions, logs and caches"
              size={12.8 * GB}
              meta="9.4 GB safe · 3.4 GB to review"
              badge={{ tone: "safe", label: "Mostly safe" }}
            />
            <SuggestionCard
              titleAs="p"
              icon={Package}
              hue="orange"
              title="Old project build files"
              description="node_modules, target and .venv in 14 projects idle for 90+ days"
              size={9.6 * GB}
              meta="14 projects · rebuilds on demand"
              badge={{ tone: "safe", label: "Rebuilds" }}
              className="max-md:hidden"
            />
            <SuggestionCard
              titleAs="p"
              icon={Terminal}
              hue="teal"
              title="Developer caches"
              description="npm, pnpm, pip, uv and Cargo download caches"
              size={6.1 * GB}
              meta="7 caches"
              badge={{ tone: "safe", label: "Safe" }}
              className="max-md:hidden"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
