// Opt-in update check (the only network call Pare ever makes). Off until the user turns it on.
import { isTauri } from "./ipc";

export interface UpdateInfo {
  version: string;
  notes: string | null;
  install: () => Promise<void>;
}

export type UpdateResult = { kind: "none" } | { kind: "available"; update: UpdateInfo } | { kind: "unavailable"; reason: string };

export async function checkForUpdate(): Promise<UpdateResult> {
  if (!isTauri) return { kind: "unavailable", reason: "Updates are checked in the installed app." };
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const u = await check();
    if (!u) return { kind: "none" };
    return {
      kind: "available",
      update: {
        version: u.version,
        notes: u.body ?? null,
        install: async () => {
          await u.downloadAndInstall();
          const { relaunch } = await import("@tauri-apps/plugin-process");
          await relaunch();
        },
      },
    };
  } catch {
    return { kind: "unavailable", reason: "This build of Pare can't update itself. Download new versions from the Releases page." };
  }
}

const LAST_CHECK = "pare.lastUpdateCheck";

/** At most once a day. */
export function dueForCheck(now = Date.now()): boolean {
  try {
    const last = Number(localStorage.getItem(LAST_CHECK) ?? 0);
    return now - last > 86_400_000;
  } catch {
    return true;
  }
}

export function markChecked(now = Date.now()) {
  try {
    localStorage.setItem(LAST_CHECK, String(now));
  } catch {
    /* storage unavailable: check again next launch */
  }
}
