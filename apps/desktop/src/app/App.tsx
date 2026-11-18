import { useEffect, useState } from "react";
import { Sidebar, TitleBar, type SidebarScreen } from "@pare/ui";
import { windowControls } from "../lib/ipc";
import { driveLabel, primaryDrive, useApp, type Screen } from "../state/app";
import { wireCleanEvents } from "../state/clean";
import { useScan, wireScanEvents } from "../state/scan";
import { Home } from "./Home";
import { Scanning } from "./Scanning";
import { Review } from "./Review";
import { Done } from "./Done";
import { History } from "./History";
import { SettingsScreen } from "./Settings";
import { Projects } from "./Projects";
import { Browse } from "./Browse";
import { Onboarding } from "./Onboarding";
import { checkForUpdate, dueForCheck, markChecked, type UpdateInfo } from "../lib/updates";
import { formatBytes } from "../lib/format";
import { useT } from "../i18n";
import { Button } from "@pare/ui";

function sidebarScreen(s: Screen): SidebarScreen {
  switch (s.name) {
    case "projects":
    case "browse":
    case "history":
    case "settings":
      return s.name;
    default:
      return "clean";
  }
}

export function App() {
  // Subscribing here re-renders the whole tree when the language changes.
  const { t } = useT();
  const screen = useApp((s) => s.screen);
  const ready = useApp((s) => s.ready);
  const drives = useApp((s) => s.drives);
  const theme = useApp((s) => s.settings?.theme ?? "system");
  const go = useApp((s) => s.go);
  const [maximized, setMaximized] = useState(false);
  const onboardingDone = useApp((s) => s.settings?.onboardingDone ?? true);
  const checkUpdates = useApp((s) => s.settings?.checkForUpdates ?? false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    if (!checkUpdates || !dueForCheck()) return;
    void checkForUpdate().then((r) => {
      markChecked();
      if (r.kind === "available") setUpdate(r.update);
    });
  }, [checkUpdates]);

  useEffect(() => {
    wireScanEvents();
    wireCleanEvents();
    void useApp
      .getState()
      .init()
      .then(() => useScan.getState().reload());
    let un: (() => void) | undefined;
    void windowControls.onResized(() => void windowControls.isMaximized().then(setMaximized)).then((u) => (un = u));
    return () => un?.();
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const drive = primaryDrive(drives);

  return (
    <div className="flex h-full flex-col bg-bg-app">
      <TitleBar
        labels={{ minimize: t("window.minimize"), maximize: t("window.maximize"), restore: t("window.restore"), close: t("window.close") }}
        maximized={maximized}
        onMinimize={() => void windowControls.minimize()}
        onMaximize={() => void windowControls.toggleMaximize()}
        onClose={() => void windowControls.close()}
      />
      <div className="flex min-h-0 flex-1">
        <Sidebar
          active={sidebarScreen(screen)}
          navLabel={t("nav.main")}
          labels={{
            clean: t("nav.clean"),
            browse: t("nav.browse"),
            projects: t("nav.projects"),
            history: t("nav.history"),
            settings: t("nav.settings"),
          }}
          drive={
            drive
              ? {
                  label: driveLabel(drive),
                  usedBytes: drive.total - drive.free,
                  totalBytes: drive.total,
                  caption: t("nav.driveUsed", { used: formatBytes(drive.total - drive.free), total: formatBytes(drive.total) }),
                  meterLabel: t("nav.driveMeter", { drive: driveLabel(drive) }),
                }
              : undefined
          }
          onNavigate={(s) => {
            if (s === "clean") go(useScan.getState().running ? { name: "scanning" } : { name: "clean" });
            else go({ name: s });
          }}
        />
        <main id="main" className="relative flex min-w-0 flex-1 flex-col overflow-y-auto overflow-x-hidden" tabIndex={-1}>
          {update && (
            <div role="status" className="flex items-center gap-3 border-b border-border-default bg-accent-subtle px-6 py-2 text-body-s text-accent-on-subtle">
              <span className="flex-1">{t("app.updateAvailable", { version: update.version })}</span>
              <Button variant="ghost" onClick={() => setUpdate(null)}>
                {t("app.later")}
              </Button>
              <Button onClick={() => void update.install()}>{t("app.installRestart")}</Button>
            </div>
          )}
          <div className="min-h-0 flex-1">{!ready ? null : <ScreenView screen={screen} />}</div>
        </main>
      </div>
      {ready && !onboardingDone && <Onboarding />}
    </div>
  );
}

function ScreenView({ screen }: { screen: Screen }) {
  switch (screen.name) {
    case "clean":
      return <Home />;
    case "scanning":
      return <Scanning />;
    case "review":
      return <Review key={screen.category} category={screen.category} />;
    case "done":
      return <Done />;
    case "history":
      return <History op={screen.op} />;
    case "settings":
      return <SettingsScreen />;
    case "projects":
      return <Projects />;
    case "browse":
      return <Browse />;
  }
}
