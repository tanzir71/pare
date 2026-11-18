import type { ReactNode } from "react";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, Chip, cx, focusRing } from "@pare/ui";
import { api, isTauri } from "../lib/ipc";
import { checkForUpdate, markChecked, type UpdateResult } from "../lib/updates";
import { useEffect, useState } from "react";
import type { SchedulableRule, Settings, Theme, Thresholds } from "../lib/types";
import { categoryTitle, comesBackLabel } from "../lib/format";
import { resolveLocale, useT, type MessageKey } from "../i18n";
import { applyLanguage, useApp } from "../state/app";

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
  const { t } = useT();
  if (paths.length === 0) return <p className="text-body-s text-text-tertiary">{empty}</p>;
  return (
    <ul className="overflow-hidden rounded-md border border-border-default">
      {paths.map((p) => (
        <li key={p} className="flex items-center gap-3 border-b border-border-default px-3 py-2 last:border-b-0">
          <span className="min-w-0 flex-1 truncate font-mono text-mono-s">{p}</span>
          <button
            type="button"
            aria-label={t("settings.remove", { path: p })}
            onClick={() => onRemove(p)}
            className={cx("rounded-md p-1 text-text-secondary hover:bg-bg-subtle", focusRing)}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </li>
      ))}
    </ul>
  );
}

function Select<T extends string | number>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 text-body-m">
      <span className="text-text-secondary">{label}</span>
      <select
        value={String(value)}
        onChange={(e) => {
          const picked = options.find((o) => String(o.value) === e.currentTarget.value);
          if (picked) onChange(picked.value);
        }}
        className={cx("h-9 rounded-md border border-border-strong bg-bg-surface px-2 text-body-m", focusRing)}
      >
        {options.map((o) => (
          <option key={String(o.value)} value={String(o.value)}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

const EVERY: { value: number; label: MessageKey }[] = [
  { value: 0, label: "settings.every.off" },
  { value: 7, label: "settings.every.week" },
  { value: 30, label: "settings.every.month" },
];

function Schedules({ settings, set }: { settings: Settings; set: (p: Partial<Settings>) => void }) {
  const { t } = useT();
  const [rules, setRules] = useState<SchedulableRule[] | null>(null);
  useEffect(() => {
    void api.schedulableRules().then(setRules).catch(() => setRules([]));
  }, []);
  if (!rules) return <p className="text-body-s text-text-tertiary">{t("common.loading")}</p>;
  const every = (id: string) => settings.schedules.find((s) => s.rule === id)?.everyDays ?? 0;
  const change = (id: string, days: number) => {
    const rest = settings.schedules.filter((s) => s.rule !== id);
    set({ schedules: days > 0 ? [...rest, { rule: id, everyDays: days }] : rest });
  };
  return (
    <ul className="overflow-hidden rounded-md border border-border-default">
      {rules.map((r) => (
        <li key={r.id} className="flex items-start gap-4 border-b border-border-default px-3 py-2.5 last:border-b-0">
          <div className="min-w-0 flex-1">
            <p className="text-body-m-strong">
              {r.tool} · {r.title}
            </p>
            <p className="text-body-s text-text-secondary">{r.why}</p>
            <p className="text-caption text-text-tertiary">
              {t("settings.schedules.detail", { ifRemoved: r.ifRemoved, comesBack: comesBackLabel(r.comesBack) })}
            </p>
          </div>
          <select
            aria-label={t("settings.schedules.for", { title: r.title })}
            value={String(every(r.id))}
            onChange={(e) => change(r.id, Number(e.currentTarget.value))}
            className={cx("h-9 shrink-0 rounded-md border border-border-strong bg-bg-surface px-2 text-body-m", focusRing)}
          >
            {EVERY.map((o) => (
              <option key={o.value} value={o.value}>
                {t(o.label)}
              </option>
            ))}
          </select>
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

const THRESHOLDS: { key: keyof Thresholds; label: MessageKey; unit: MessageKey; min: number }[] = [
  { key: "largeFileMb", label: "settings.threshold.largeFileMb", unit: "settings.unit.mb", min: 1 },
  { key: "largeFileDays", label: "settings.threshold.largeFileDays", unit: "settings.unit.days", min: 1 },
  { key: "oldDownloadDays", label: "settings.threshold.oldDownloadDays", unit: "settings.unit.days", min: 1 },
  { key: "installerDays", label: "settings.threshold.installerDays", unit: "settings.unit.days", min: 0 },
  { key: "idleDays", label: "settings.threshold.idleDays", unit: "settings.unit.days", min: 1 },
  { key: "screenshotDays", label: "settings.threshold.screenshotDays", unit: "settings.unit.days", min: 1 },
  { key: "duplicateMinMb", label: "settings.threshold.duplicateMinMb", unit: "settings.unit.mb", min: 0 },
];

/** Language setting values: "" follows Windows. */
const LANGUAGES: { value: string; label: MessageKey }[] = [
  { value: "", label: "settings.language.system" },
  { value: "en", label: "settings.language.en" },
  { value: "es", label: "settings.language.es" },
];

type UpdateState = { kind: "checking" } | UpdateResult;

export function SettingsScreen() {
  const { t, tp } = useT();
  const settings = useApp((s) => s.settings);
  const info = useApp((s) => s.info);
  const update = useApp((s) => s.updateSettings);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  if (!settings) return null;
  const set = (patch: Partial<Settings>) => void update(patch);

  async function pickFolder(title: string): Promise<string | null> {
    if (!isTauri) return null;
    const picked = await openDialog({ directory: true, multiple: false, title });
    return typeof picked === "string" ? picked : null;
  }

  function updateMessage(s: UpdateState): string {
    switch (s.kind) {
      case "checking":
        return t("common.checking");
      case "none":
        return t("settings.updates.latest");
      case "available":
        return t("settings.updates.available", { version: s.update.version });
      case "unavailable":
        return s.reason === "preview" ? t("settings.updates.preview") : t("settings.updates.unsupported");
    }
  }

  // A stored tag like "es-MX" shows as its language; "" stays "Match Windows".
  const language = settings.language.trim() === "" ? "" : resolveLocale(settings.language);

  return (
    <div className="mx-auto flex max-w-[760px] flex-col gap-5 pb-12 pl-4 pr-8 pt-6">
      <h1 className="text-heading-h1">{t("settings.title")}</h1>

      <Section title={t("settings.locations.title")} description={t("settings.locations.description")}>
        <PathList
          paths={settings.customRoots.map((r) => r.path)}
          empty={t("settings.locations.empty")}
          onRemove={(p) => set({ customRoots: settings.customRoots.filter((r) => r.path !== p) })}
        />
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder(t("common.dialog.addScanFolder"));
              if (p && !settings.customRoots.some((r) => r.path === p)) set({ customRoots: [...settings.customRoots, { path: p, label: p }] });
            }}
          >
            {t("common.addFolder")}
          </Button>
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder(t("common.dialog.addProjectsFolder"));
              if (p && !settings.projectRoots.includes(p)) set({ projectRoots: [...settings.projectRoots, p] });
            }}
          >
            {t("settings.addProjectsFolder")}
          </Button>
        </div>
        {settings.projectRoots.length > 0 && (
          <PathList paths={settings.projectRoots} empty="" onRemove={(p) => set({ projectRoots: settings.projectRoots.filter((x) => x !== p) })} />
        )}
      </Section>

      <Section title={t("settings.keep.title")} description={t("settings.keep.description")}>
        <PathList
          paths={settings.exclusions}
          empty={t("settings.keep.empty")}
          onRemove={(p) => set({ exclusions: settings.exclusions.filter((x) => x !== p) })}
        />
        <div>
          <Button
            variant="secondary"
            icon={FolderPlus}
            disabled={!isTauri}
            onClick={async () => {
              const p = await pickFolder(t("settings.keep.dialog"));
              if (p && !settings.exclusions.includes(p)) set({ exclusions: [...settings.exclusions, p] });
            }}
          >
            {t("settings.keep.add")}
          </Button>
        </div>
      </Section>

      <Section title={t("settings.thresholds.title")} description={t("settings.thresholds.description")}>
        <div className="grid grid-cols-1 gap-x-6 gap-y-3 min-[900px]:grid-cols-2">
          {THRESHOLDS.map((th) => (
            <label key={th.key} className="flex items-center justify-between gap-3 text-body-m">
              <span className="text-text-secondary">{t(th.label)}</span>
              <span className="flex items-center gap-2">
                <input
                  type="number"
                  min={th.min}
                  value={settings.thresholds[th.key]}
                  onChange={(e) => {
                    const v = Math.max(th.min, Number(e.currentTarget.value) || th.min);
                    set({ thresholds: { ...settings.thresholds, [th.key]: v } });
                  }}
                  className={cx("tabular h-9 w-24 rounded-md border border-border-strong bg-bg-surface px-2 text-right text-body-m", focusRing)}
                />
                <span className="w-9 text-text-tertiary">{t(th.unit)}</span>
              </span>
            </label>
          ))}
        </div>
      </Section>

      <Section title={t("settings.disposal.title")}>
        <Toggle
          label={t("settings.cachesToBin.label")}
          hint={t("settings.cachesToBin.hint")}
          checked={settings.cachesToRecycleBin}
          onChange={(v) => set({ cachesToRecycleBin: v })}
        />
        <Toggle
          label={t("settings.purge.label")}
          hint={t("settings.purge.hint")}
          checked={settings.purgePareItemsAfterDays != null}
          onChange={(v) => set({ purgePareItemsAfterDays: v ? 30 : null })}
        />
      </Section>

      <Section title={t("settings.monitor.title")} description={t("settings.monitor.description")}>
        <Toggle
          label={t("settings.monitor.watch.label")}
          hint={t("settings.monitor.watch.hint")}
          checked={settings.monitor.enabled}
          onChange={(v) => set({ monitor: { ...settings.monitor, enabled: v } })}
        />
        <Toggle
          label={t("settings.monitor.tray.label")}
          hint={t("settings.monitor.tray.hint")}
          checked={settings.monitor.tray}
          onChange={(v) => set({ monitor: { ...settings.monitor, tray: v } })}
        />
        {settings.monitor.enabled && (
          <div className="grid grid-cols-1 gap-x-6 gap-y-3 min-[900px]:grid-cols-2">
            <Select
              label={t("settings.monitor.alert")}
              value={settings.monitor.alertGb}
              options={[1, 2, 5, 10, 20].map((n) => ({ value: n, label: t("settings.monitor.alertOption", { n }) }))}
              onChange={(v) => set({ monitor: { ...settings.monitor, alertGb: v } })}
            />
            <Select
              label={t("settings.monitor.every")}
              value={settings.monitor.checkHours}
              options={[3, 6, 12, 24].map((n) => ({ value: n, label: tp("time.hours", n) }))}
              onChange={(v) => set({ monitor: { ...settings.monitor, checkHours: v } })}
            />
          </div>
        )}
      </Section>

      <Section title={t("settings.schedules.title")} description={t("settings.schedules.description")}>
        <Schedules settings={settings} set={set} />
      </Section>

      <Section title={t("settings.appearance.title")}>
        <div className="flex gap-2" role="radiogroup" aria-label={t("settings.theme")}>
          {(["system", "light", "dark"] as Theme[]).map((th) => (
            <Chip key={th} selected={settings.theme === th} role="radio" aria-checked={settings.theme === th} onClick={() => set({ theme: th })}>
              {t(`settings.theme.${th}`)}
            </Chip>
          ))}
        </div>
        <Select
          label={t("settings.language")}
          value={language}
          options={LANGUAGES.map((l) => ({ value: l.value, label: t(l.label) }))}
          onChange={(v) => {
            // Switch the UI right away, then save; the backend follows from the next scan.
            applyLanguage(v);
            set({ language: v });
          }}
        />
        {settings.hiddenCards.length > 0 && (
          <div>
            <p className="text-body-m-strong">{t("settings.hidden")}</p>
            <div className="mt-2 flex flex-wrap gap-2">
              {settings.hiddenCards.map((c) => (
                <Chip key={c} onClick={() => set({ hiddenCards: settings.hiddenCards.filter((x) => x !== c) })}>
                  {t("settings.show", { title: categoryTitle(c) })}
                </Chip>
              ))}
            </div>
          </div>
        )}
      </Section>

      <Section title={t("settings.updates.title")} description={t("settings.updates.description")}>
        <Toggle
          label={t("settings.updates.check.label")}
          hint={t("settings.updates.check.hint")}
          checked={settings.checkForUpdates}
          onChange={(v) => set({ checkForUpdates: v })}
        />
        {settings.checkForUpdates && (
          <div className="flex items-center gap-3">
            <Button
              variant="secondary"
              onClick={async () => {
                setUpdateState({ kind: "checking" });
                const r = await checkForUpdate();
                markChecked();
                setUpdateState(r);
              }}
            >
              {t("settings.updates.checkNow")}
            </Button>
            {updateState && <span className="text-body-s text-text-secondary">{updateMessage(updateState)}</span>}
          </div>
        )}
      </Section>

      <Section title={t("settings.about.title")}>
        <p className="text-body-m">{t("settings.about.line", { version: info?.version ?? "" })}</p>
        <p className="text-body-s text-text-secondary">{t("settings.about.disclaimer")}</p>
        <div>
          <Button variant="secondary" icon={FolderOpen} onClick={() => info && void api.openInExplorer(info.logDir)}>
            {t("settings.about.openLogs")}
          </Button>
        </div>
      </Section>
    </div>
  );
}
