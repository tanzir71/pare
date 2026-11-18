//! Data model shared by the scanner, the safety pass, disposal and the UI (serialized as JSON).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Milliseconds since the Unix epoch.
pub type Timestamp = i64;

pub const MS_PER_DAY: i64 = 86_400_000;
pub const MS_PER_HOUR: i64 = 3_600_000;

/// Suggestion card a finding belongs to.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum CategoryId {
    AiTools,
    ProjectBuild,
    DevCaches,
    LargeFiles,
    Downloads,
    RecycleBin,
    Duplicates,
    Junk,
    Browsers,
    Screenshots,
    EmptyFolders,
}

impl CategoryId {
    pub const ALL: [CategoryId; 11] = [
        CategoryId::AiTools,
        CategoryId::ProjectBuild,
        CategoryId::DevCaches,
        CategoryId::LargeFiles,
        CategoryId::Downloads,
        CategoryId::RecycleBin,
        CategoryId::Duplicates,
        CategoryId::Junk,
        CategoryId::Browsers,
        CategoryId::Screenshots,
        CategoryId::EmptyFolders,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CategoryId::AiTools => "ai-tools",
            CategoryId::ProjectBuild => "project-build",
            CategoryId::DevCaches => "dev-caches",
            CategoryId::LargeFiles => "large-files",
            CategoryId::Downloads => "downloads",
            CategoryId::RecycleBin => "recycle-bin",
            CategoryId::Duplicates => "duplicates",
            CategoryId::Junk => "junk",
            CategoryId::Browsers => "browsers",
            CategoryId::Screenshots => "screenshots",
            CategoryId::EmptyFolders => "empty-folders",
        }
    }

    pub fn info(self) -> CategoryInfo {
        let (title, description, icon, hue) = match self {
            CategoryId::AiTools => (
                "AI tool caches",
                "Claude Code, Codex and Cursor sessions, logs and caches",
                "Sparkles",
                "violet",
            ),
            CategoryId::ProjectBuild => (
                "Old project build files",
                "node_modules, target and .venv in idle projects",
                "Package",
                "orange",
            ),
            CategoryId::DevCaches => (
                "Developer caches",
                "npm, pnpm, pip, uv and Cargo download caches",
                "Terminal",
                "teal",
            ),
            CategoryId::LargeFiles => (
                "Large files",
                "Files over 500 MB you haven't changed in months",
                "File",
                "pink",
            ),
            CategoryId::Downloads => (
                "Downloads",
                "Installers and archives you've already used",
                "Download",
                "blue",
            ),
            CategoryId::RecycleBin => (
                "Recycle Bin",
                "Files you already deleted, still taking up space",
                "Trash2",
                "slate",
            ),
            CategoryId::Duplicates => (
                "Duplicates",
                "Exact copies of the same file",
                "Copy",
                "amber",
            ),
            CategoryId::Junk => (
                "Junk & temp files",
                "Windows temp files, crash dumps and thumbnail cache",
                "Archive",
                "green",
            ),
            CategoryId::Browsers => (
                "Browser caches",
                "Cached pages and scripts from browsers that are closed",
                "Monitor",
                "blue",
            ),
            CategoryId::Screenshots => (
                "Old screenshots",
                "Screenshots older than 3 months in Pictures",
                "Image",
                "blue",
            ),
            CategoryId::EmptyFolders => (
                "Empty folders",
                "Folders with nothing inside",
                "FolderOpen",
                "slate",
            ),
        };
        CategoryInfo {
            id: self,
            title: title.into(),
            description: description.into(),
            icon: icon.into(),
            hue: hue.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInfo {
    pub id: CategoryId,
    pub title: String,
    pub description: String,
    /// lucide-react icon name (see docs/DESIGN.md §1).
    pub icon: String,
    /// Category hue token: `cat/<hue>/fg|bg`.
    pub hue: String,
}

/// Why something is Protected. Protected is always computed, never declared by a rule.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectReason {
    /// Modified within the recency window (24 h by default).
    RecentlyModified,
    /// Held open by a process.
    InUse,
    /// A tool that owns the files is running and the rule requires it to be closed.
    ToolRunning,
    /// Inside a git repo with uncommitted changes.
    Uncommitted,
    /// OS, system or hard-excluded path.
    SystemPath,
    /// Cloud-only placeholder (would be downloaded if read).
    CloudOnly,
    /// The user chose "Always keep this".
    Excluded,
    /// Changed between the scan and the clean.
    ChangedSinceScan,
    /// Matched a rule's `[[protect]]` never-touch list.
    NeverTouch,
}

impl ProtectReason {
    /// Short badge label ("Keep", "In use", "Uncommitted").
    pub fn badge(self) -> &'static str {
        match self {
            ProtectReason::InUse | ProtectReason::ToolRunning => "In use",
            ProtectReason::Uncommitted => "Uncommitted",
            ProtectReason::RecentlyModified => "Recent",
            ProtectReason::CloudOnly => "Cloud only",
            _ => "Keep",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            ProtectReason::RecentlyModified => "Changed in the last 24 hours",
            ProtectReason::InUse => "Open in another app",
            ProtectReason::ToolRunning => "The app that owns it is running",
            ProtectReason::Uncommitted => "Part of uncommitted work in a git repository",
            ProtectReason::SystemPath => "A Windows or system location Pare never touches",
            ProtectReason::CloudOnly => "Stored in the cloud; removing it locally frees nothing",
            ProtectReason::Excluded => "You chose to always keep this",
            ProtectReason::ChangedSinceScan => "Changed since the scan",
            ProtectReason::NeverTouch => "Settings or credentials Pare never touches",
        }
    }
}

/// Safety tier. Signals can only move a finding *down* this list (Safe → Review → Protected).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Tier {
    Safe,
    Review,
    Protected {
        reason: ProtectReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

impl Tier {
    pub fn rank(&self) -> u8 {
        match self {
            Tier::Safe => 0,
            Tier::Review => 1,
            Tier::Protected { .. } => 2,
        }
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Tier::Protected { .. })
    }

    pub fn is_selectable(&self) -> bool {
        !self.is_protected()
    }

    pub fn protected(reason: ProtectReason) -> Tier {
        Tier::Protected {
            reason,
            detail: None,
        }
    }

    /// Downgrade-only: returns the safer (higher-rank) of `self` and `other`.
    #[must_use]
    pub fn downgrade(self, other: Tier) -> Tier {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }
}

/// Does the data come back after removal?
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ComesBack {
    /// "Yes, automatically" — the app rebuilds it.
    Automatically,
    /// "Re-downloads when needed".
    OnDemand,
    /// "No".
    No,
}

impl ComesBack {
    pub fn label(self) -> &'static str {
        match self {
            ComesBack::Automatically => "Yes, automatically",
            ComesBack::OnDemand => "Re-downloads when needed",
            ComesBack::No => "No",
        }
    }
}

/// How an item is disposed of.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Disposal {
    /// Permanent delete. Only for rebuildable data.
    Delete,
    /// Move to the Recycle Bin (default).
    Recycle,
    /// Run the tool's own cleanup command.
    Command { program: String, args: Vec<String> },
    /// Pare doesn't remove it: show steps. Some steps have an action Pare can run on an
    /// explicit click (M8: Docker/WSL compaction, Windows Update cleanup).
    Guide {
        steps: Vec<String>,
        #[serde(default)]
        actions: Vec<GuideAction>,
    },
    /// Empty the Recycle Bin of one drive (e.g. `C:\`).
    EmptyRecycleBin { drive: String },
}

impl Disposal {
    pub fn kind(&self) -> &'static str {
        match self {
            Disposal::Delete => "delete",
            Disposal::Recycle => "recycle",
            Disposal::Command { .. } => "command",
            Disposal::Guide { .. } => "guide",
            Disposal::EmptyRecycleBin { .. } => "empty-recycle-bin",
        }
    }

    /// Does this disposal free space immediately?
    pub fn frees_now(&self) -> bool {
        matches!(
            self,
            Disposal::Delete | Disposal::Command { .. } | Disposal::EmptyRecycleBin { .. }
        )
    }

    pub fn is_actionable(&self) -> bool {
        !matches!(self, Disposal::Guide { .. })
    }
}

/// A guide step Pare can run for the user, only when they click it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuideAction {
    /// Index into the guide's steps.
    pub step: usize,
    /// Button label, e.g. "Prune unused Docker data".
    pub label: String,
    /// Program and arguments. `pare:compact-vhd` compacts the finding's virtual disks.
    pub run: Vec<String>,
    /// Needs Pare to run as administrator. Pare never asks for it on its own; without it the
    /// UI shows the command to run in an administrator terminal.
    pub admin: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dates {
    pub created_min: Option<Timestamp>,
    pub modified_max: Option<Timestamp>,
    pub accessed_max: Option<Timestamp>,
}

impl Dates {
    pub fn include(&mut self, created: Timestamp, modified: Timestamp, accessed: Timestamp) {
        if created > 0 {
            self.created_min = Some(self.created_min.map_or(created, |c| c.min(created)));
        }
        if modified > 0 {
            self.modified_max = Some(self.modified_max.map_or(modified, |m| m.max(modified)));
        }
        if accessed > 0 {
            self.accessed_max = Some(self.accessed_max.map_or(accessed, |a| a.max(accessed)));
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub repo: PathBuf,
    pub branch: Option<String>,
    pub last_commit: Option<Timestamp>,
    pub commits: u64,
    pub dirty: bool,
    pub dirty_files: u64,
    pub ahead: u64,
    pub has_remote: bool,
    pub has_upstream: bool,
    pub stashes: u64,
    pub detached: bool,
}

/// One selectable item: a file, a folder, or a logical group.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    /// e.g. `quick.large-files`, `claude-code.debug-logs`.
    pub rule_id: String,
    pub category: CategoryId,
    /// Group header: tool name or folder.
    pub source: String,
    /// Path shown next to the group header.
    pub source_path: Option<PathBuf>,
    pub title: String,
    pub paths: Vec<PathBuf>,
    /// Allocated bytes Pare can actually reclaim (hard links counted once, and only when every
    /// link is inside this finding).
    pub bytes: u64,
    /// Logical (file-size) bytes, for tooltips.
    pub logical_bytes: u64,
    /// Number of files.
    pub items: u64,
    pub tier: Tier,
    pub disposal: Disposal,
    pub comes_back: ComesBack,
    pub why: String,
    pub if_removed: String,
    pub dates: Dates,
    pub created_by: Option<String>,
    /// Custom badge label ("Orphaned", "Old versions", "Rebuilds"); the tier label otherwise.
    pub badge: Option<String>,
    /// Short "age" / status line shown in the row ("9 days ago", "Folders deleted").
    pub note: Option<String>,
    pub git: Option<GitInfo>,
    /// Extra facts for the details panel (label, value).
    #[serde(default)]
    pub facts: Vec<(String, String)>,
}

impl Finding {
    pub fn is_selectable(&self) -> bool {
        self.tier.is_selectable() && self.disposal.is_actionable()
    }
}

/// Per-card summary streamed to the UI.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    #[serde(flatten)]
    pub info: CategoryInfo,
    pub safe_bytes: u64,
    pub review_bytes: u64,
    pub protected_bytes: u64,
    pub items: u64,
    pub findings: u64,
    pub sources: u64,
    pub protected_findings: u64,
    /// Bytes that disposal frees immediately vs. sends to the Recycle Bin (selectable only).
    pub frees_now_bytes: u64,
    pub recycle_bytes: u64,
}

impl CategorySummary {
    pub fn total_bytes(&self) -> u64 {
        self.safe_bytes + self.review_bytes
    }
}

/// A process holding a file open.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessHolder {
    pub pid: u32,
    pub name: String,
}
