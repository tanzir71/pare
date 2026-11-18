// One place for links and launch state. Flip RELEASED to true when v0.1.0 is on GitHub Releases:
// every CTA then reads "Download for Windows" and points at releases/latest.
import { Bell, Download, type LucideIcon } from "lucide-react";

export const RELEASED = false;

/** Installer size of the published release (measured: v0.1.0 release build from CI, 3,196,614 bytes). Shown only
 * once RELEASED is true, because landing numbers must come from the shipped app. */
export const DOWNLOAD_SIZE = "3 MB";
export const downloadSizeText = RELEASED ? `A ${DOWNLOAD_SIZE} download.` : "A small download.";

export const REPO_URL = "https://github.com/tanzir71/pare";
export const RELEASES_URL = `${REPO_URL}/releases/latest`;
/** Before the first release, "Get notified" sends people to the README's Install section (watch the repo there). */
export const NOTIFY_URL = `${REPO_URL}?tab=readme-ov-file#install`;

export const download: { label: string; shortLabel: string; href: string; icon: LucideIcon } =
  RELEASED
    ? { label: "Download for Windows", shortLabel: "Download", href: RELEASES_URL, icon: Download }
    : { label: "Get notified", shortLabel: "Get notified", href: NOTIFY_URL, icon: Bell };

export const links = {
  repo: REPO_URL,
  changelog: `${import.meta.env.BASE_URL.replace(/\/?$/, "/")}changelog/`,
  roadmap: `${REPO_URL}/blob/main/docs/MILESTONES.md`,
  rules: `${REPO_URL}/tree/main/rules`,
  contribute: `${REPO_URL}?tab=readme-ov-file#contribute-a-cleanup-rule`,
  security: `${REPO_URL}/blob/main/SECURITY.md`,
  license: `${REPO_URL}/blob/main/LICENSE`,
} as const;

export const site = {
  name: "Pare",
  title: "Pare: free up space on Windows, safely",
  description:
    "Pare is a free, open-source Windows cleanup app. It finds caches, idle build folders, AI-tool logs and forgotten downloads, and shows you what's safe to remove before anything is deleted.",
  tagline: "Free up space on Windows — safely.",
} as const;

/** Prefix a site-relative path with Astro's `base` ("/pare"). */
export function withBase(path: string): string {
  const base = import.meta.env.BASE_URL.replace(/\/$/, "");
  return `${base}/${path.replace(/^\//, "")}`;
}
