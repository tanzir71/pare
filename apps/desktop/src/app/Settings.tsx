import type { ReactNode } from "react";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, Chip, cx, focusRing } from "@pare/ui";
import { api, isTauri } from "../lib/ipc";
import type { CategoryId, Settings, Theme, Thresholds } from "../lib/types";
import { useApp } from "../state/app";

function Section({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-border-default bg-bg-surface p-5">
      <h2 className="text-heading-h3">{title}</h2>
      {description && <p className="mt-1 text-body-s text-text-secondary">{description}</p>}
      <div className="mt-4 flex flex-col gap-3">{children}</div>
    </section>
  );
}

function PathList({ paths, onRemove, empty }: { paths: string[]; onRemove: (p: string) => void; empty: string }) {
  if (paths.length === 0) return <p className="text-body-s text-text-tertiary">{empty}</p>;
  return (
    <ul className="overflow-hidden rounded-md border border-border-default">
      {paths.map((p) => (
        <li key={p} className="flex items-center gap-3 border-b border-border-default px-3 py-2 last:border-b-0">
          <span className="min-w-0 flex-1 truncate font-mono text-mono-s">{p}</span>
          <button type="button" aria-label={`Remove ${p}`} onClick={() => onRemove(p)} className={cx("rounded-md p-1 text-text-secondary hover:bg-bg-subtle", focusRing)}>
            <X size={16} aria-hidden="true" />
          </button>
        </li>
      ))}
    </ul>
  );
}

function Toggle({ checked, onChange, label, hint }: { checked: boolean; onChange: (v: boolean) => void; label: string; hint?: string }) {
  return (
    <label className="flex cursor-pointer items-start justify-between gap-4">
      <span>
        <span className="block text-body-m-strong">{label}</span>
        {hint && <span className="block text-body-s text-text-secondary">{hint}</span>}
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={checked}
        onChange={(e) => onChange(e.currentTarget.checked)}
        className={cx(
          "relative mt-0.5 h-6 w-10 shrink-0 cursor-pointer appearance-none rounded-full bg-border-strong transition-colors",
          "before:absolute before:left-0.5 before:top-0.5 before:size-5 before:rounded-full before:bg-bg-surface before:transition-transform",
          "checked:bg-accent-default checked:before:translate-x-4",
          focusRing,
        )}
      />
    </label>
  );
}

const THRESHOLDS: { key: keyof Thresholds; label: string; unit: string; min: number }[] = [
  { key: "largeFileMb", label: "Large files are at least", unit: "MB", min: 1 },
  { key: "largeFileDays", label: "…and unchanged for", unit: "days", min: 1 },
  { key: "oldDownloadDays", label: "Old downloads: unchanged for", unit: "days", min: 1 },
  { key: "installerDays", label: "Installers: older than", unit: "days", min: 0 },
  { key: "idleDays", label: "Projects are idle after", unit: "days", min: 1 },
  { key: "screenshotDays", label: "Screenshots: older than", unit: "days", min: 1 },
  { key: "duplicateMinMb", label: "Duplicates: skip files under", unit: "MB", min: 0 },
];

const CARD_TITLES: Record<CategoryId, string> = {
  "ai-tools": "AI tool caches",
  "project-build": "Old project build files",
  "dev-caches": "Developer caches",
  "large-files": "Large files",
  downloads: "Downloads",
  "recycle-bin": "Recycle Bin",
  duplicates: "Duplicates",
  junk: "Junk & temp files",
  browsers: "Browser caches",
  screenshots: "Old screenshots",
  "empty-folders": "Empty folders",
};

export function SettingsScreen() {
  const settings = useApp((s) => s.settings);
  const info = useApp((s) => s.info);
  const update = useApp((s) => s.updateSettings);
  if (!settings) return null;
  const set = (patch: Partial<Settings>) => void update(patch);

  async function pickFolder(title: string): Promise<string | null> {
    if (!isTauri) return null;
    const picked = await openDialog({ directory: true, multiple: false, title });
    return typeof picked === "string" ? picked : null;
  }

  return (
    <div className="mx-auto flex max-w-[760px] flex-col gap-5 pb-12 pl-4 pr-8 pt-6">
      <h1 className="text-heading-h1">Settings</h1>

      <Section title="Scan locations" description="Folders that appear as scope chips on Clean, and where Pare looks for git projects.">
        <PathList
          paths={settings.customRoots.map((r) => r.path)}
          empty="No extra folders yet."
          onRemove={(p) => set({ customRoots: settings.customRoots.filter((r) => r.path !== p) })}
        />
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder("Add a folder to scan");
              if (p && !settings.customRoots.some((r) => r.path === p)) set({ customRoots: [...settings.customRoots, { path: p, label: p }] });
            }}
          >
            Add folder
          </Button>
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder("Add a projects folder");
              if (p && !settings.projectRoots.includes(p)) set({ projectRoots: [...settings.projectRoots, p] });
            }}
          >
            Add projects folder
          </Button>
        </div>
        {settings.projectRoots.length > 0 && (
          <PathList paths={settings.projectRoots} empty="" onRemove={(p) => set({ projectRoots: settings.projectRoots.filter((x) => x !== p) })} />
        )}
      </Section>

      <Section title="Always keep" description="Pare never suggests these, and anything inside them.">
        <PathList paths={settings.exclusions} empty="Nothing yet. Use “Always keep this” in a suggestion's details." onRemove={(p) => set({ exclusions: settings.exclusions.filter((x) => x !== p) })} />
        <div>
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder("Always keep this folder");
              if (p && !settings.exclusions.includes(p)) set({ exclusions: [...settings.exclusions, p] });
            }}
          >
            Add a folder to keep
          </Button>
        </div>
      </Section>

      <Section title="Thresholds" description="What counts as large, old or idle.">
        <div className="grid grid-cols-1 gap-x-6 gap-y-3 min-[900px]:grid-cols-2">
          {THRESHOLDS.map((t) => (
            <label key={t.key} className="flex items-center justify-between gap-3 text-body-m">
              <span className="text-text-secondary">{t.label}</span>
              <span className="flex items-center gap-2">
                <input
                  type="number"
                  min={t.min}
                  value={settings.thresholds[t.key]}
                  onChange={(e) => {
                    const v = Math.max(t.min, Number(e.currentTarget.value) || t.min);
                    set({ thresholds: { ...settings.thresholds, [t.key]: v } });
                  }}
                  className={cx("tabular h-9 w-24 rounded-md border border-border-strong bg-bg-surface px-2 text-right text-body-m", focusRing)}
                />
                <span className="w-9 text-text-tertiary">{t.unit}</span>
              </span>
            </label>
          ))}
        </div>
      </Section>

      <Section title="Disposal">
        <Toggle
          label="Send caches to the Recycle Bin too"
          hint="Normally caches that rebuild themselves are removed right away. With this on, everything goes to the Recycle Bin first (frees space only when the bin is emptied)."
          checked={settings.cachesToRecycleBin}
          onChange={(v) => set({ cachesToRecycleBin: v })}
        />
        <Toggle
          label="Empty Pare's items from the Recycle Bin after 30 days"
          hint="Only items Pare moved. Everything else in your Recycle Bin is left alone."
          checked={settings.purgePareItemsAfterDays != null}
          onChange={(v) => set({ purgePareItemsAfterDays: v ? 30 : null })}
        />
      </Section>

      <Section title="Appearance">
        <div className="flex gap-2" role="radiogroup" aria-label="Theme">
          {(["system", "light", "dark"] as Theme[]).map((t) => (
            <Chip key={t} selected={settings.theme === t} role="radio" aria-checked={settings.theme === t} onClick={() => set({ theme: t })}>
              {t === "system" ? "Match Windows" : t === "light" ? "Light" : "Dark"}
            </Chip>
          ))}
        </div>
        <label className="flex items-center justify-between text-body-m">
          <span className="text-text-secondary">Language</span>
          <select disabled value={settings.language} className="h-9 rounded-md border border-border-strong bg-bg-surface px-2 text-body-m">
            <option value="en">English</option>
          </select>
        </label>
        {settings.hiddenCards.length > 0 && (
          <div>
            <p className="text-body-m-strong">Hidden suggestions</p>
            <div className="mt-2 flex flex-wrap gap-2">
              {settings.hiddenCards.map((c) => (
                <Chip key={c} onClick={() => set({ hiddenCards: settings.hiddenCards.filter((x) => x !== c) })}>
                  Show {CARD_TITLES[c]}
                </Chip>
              ))}
            </div>
          </div>
        )}
      </Section>

      <Section title="Updates" description="Pare never connects to the internet unless you turn this on.">
        <Toggle
          label="Check for updates"
          hint="Once a day, Pare asks GitHub whether a newer version exists. Nothing about your PC is sent."
          checked={settings.checkForUpdates}
          onChange={(v) => set({ checkForUpdates: v })}
        />
      </Section>

      <Section title="About">
        <p className="text-body-m">
          Pare {info?.version} · MIT License · free and open source
        </p>
        <p className="text-body-s text-text-secondary">
          Pare is an independent project and is not affiliated with Microsoft, Google, Anthropic or OpenAI. Product names are used only to describe the files their apps create.
        </p>
        <div>
          <Button variant="secondary" icon={FolderOpen} onClick={() => info && void api.openInExplorer(info.logDir)}>
            Open log folder
          </Button>
        </div>
      </Section>
    </div>
  );
}
