import {
  Archive,
  Copy,
  Download,
  File,
  FileText,
  Folder,
  FolderOpen,
  GitBranch,
  Image,
  Info,
  Lock,
  Monitor,
  Package,
  Sparkles,
  Terminal,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import type { CategoryHue } from "@pare/ui";
import type { CategoryId, Finding, Hue } from "../lib/types";

const BY_NAME: Record<string, LucideIcon> = {
  Sparkles,
  Package,
  Terminal,
  File,
  Download,
  Trash2,
  Copy,
  Archive,
  Monitor,
  Image,
  FolderOpen,
  Folder,
  GitBranch,
};

export function iconByName(name: string): LucideIcon {
  return BY_NAME[name] ?? FileText;
}

export function hue(h: Hue | string): CategoryHue {
  return h as CategoryHue;
}

const CATEGORY_HUE: Record<CategoryId, CategoryHue> = {
  "ai-tools": "violet",
  "project-build": "orange",
  "dev-caches": "teal",
  "large-files": "pink",
  downloads: "blue",
  "recycle-bin": "slate",
  duplicates: "amber",
  junk: "green",
  browsers: "blue",
  screenshots: "blue",
  "empty-folders": "slate",
};

export function categoryHue(id: CategoryId): CategoryHue {
  return CATEGORY_HUE[id];
}

/** Row icon: lock for protected, info for guides, folder for groups of files, else file. */
export function findingIcon(f: Finding): LucideIcon {
  if (f.tier.kind === "protected") return Lock;
  if (f.disposal.kind === "guide") return Info;
  if (f.disposal.kind === "empty-recycle-bin") return Trash2;
  if (f.category === "project-build") return Package;
  return f.items > 1 || f.items === 0 ? Folder : File;
}
