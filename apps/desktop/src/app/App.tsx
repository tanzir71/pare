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
import { ComingSoon } from "./ComingSoon";
import { Projects } from "./Projects";

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
  const screen = useApp((s) => s.screen);
  const ready = useApp((s) => s.ready);
  const drives = useApp((s) => s.drives);
  const theme = useApp((s) => s.settings?.theme ?? "system");
  const go = useApp((s) => s.go);
  const [maximized, setMaximized] = useState(false);

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
        maximized={maximized}
        onMinimize={() => void windowControls.minimize()}
        onMaximize={() => void windowControls.toggleMaximize()}
        onClose={() => void windowControls.close()}
      />
      <div className="flex min-h-0 flex-1">
        <Sidebar
          active={sidebarScreen(screen)}
          drive={drive ? { label: driveLabel(drive), usedBytes: drive.total - drive.free, totalBytes: drive.total } : undefined}
          onNavigate={(s) => {
            if (s === "clean") go(useScan.getState().running ? { name: "scanning" } : { name: "clean" });
            else go({ name: s });
          }}
        />
        <main id="main" className="relative min-w-0 flex-1 overflow-y-auto overflow-x-hidden" tabIndex={-1}>
          {!ready ? null : <ScreenView screen={screen} />}
        </main>
      </div>
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
      return <ComingSoon screen="browse" />;
  }
}
