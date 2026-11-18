//! Rule engine (M3). Rules are data in `rules/*.toml`, embedded at build time, plus an optional
//! user rules folder that can add rules but never loosen built-in tiers (docs/RULES.md).

pub mod engine;
pub mod matcher;
pub mod vars;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::types::{CategoryId, ComesBack};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub processes: Vec<String>,
    #[serde(default)]
    pub docs: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum TierDecl {
    Safe,
    Review,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DisposalDecl {
    Delete,
    Recycle,
    Command,
    Guide,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuleDef {
    pub id: String,
    pub title: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub min_age_days: Option<u32>,
    #[serde(default)]
    pub keep_newest: u32,
    pub tier: TierDecl,
    /// Missing → Recycle Bin (the safe default).
    #[serde(default = "default_disposal")]
    pub disposal: DisposalDecl,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub guide: Vec<String>,
    pub comes_back: ComesBack,
    #[serde(default)]
    pub why: String,
    #[serde(default)]
    pub if_removed: String,
    #[serde(default)]
    pub requires_closed: bool,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub detector: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub sources: Vec<String>,
}

fn default_disposal() -> DisposalDecl {
    DisposalDecl::Recycle
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProtectDef {
    pub paths: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EcosystemDef {
    pub id: String,
    pub markers: Vec<String>,
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub rebuild: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    pub tool: Option<ToolDef>,
    #[serde(default)]
    pub rule: Vec<RuleDef>,
    #[serde(default)]
    pub protect: Vec<ProtectDef>,
    #[serde(default)]
    pub ecosystem: Vec<EcosystemDef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedTool {
    pub tool: ToolDef,
    pub rules: Vec<RuleDef>,
    pub file: String,
    pub builtin: bool,
}

/// Everything loaded: tools with their rules, global never-touch patterns, project ecosystems.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleSet {
    pub tools: Vec<LoadedTool>,
    /// `[[protect]]` patterns from every file (unexpanded).
    pub protect: Vec<String>,
    pub ecosystems: Vec<EcosystemDef>,
    /// Problems found while loading user rules (shown in Settings, never fatal).
    pub warnings: Vec<String>,
}

macro_rules! builtin {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../../../rules/", $name)))),*]
    };
}

/// Built-in rule files, embedded at compile time.
pub const BUILTIN: &[(&str, &str)] = builtin![
    "browsers.toml",
    "claude-code.toml",
    "claude-desktop.toml",
    "codex.toml",
    "editors.toml",
    "local-models.toml",
    "node.toml",
    "projects.toml",
    "python.toml",
    "rust-go-jvm-dotnet.toml",
    "windows.toml",
];

impl RuleSet {
    /// Built-in rules only.
    pub fn builtin() -> RuleSet {
        let mut set = RuleSet::default();
        for (name, text) in BUILTIN {
            let file: RuleFile = toml::from_str(text)
                .unwrap_or_else(|e| panic!("built-in rule file {name} is invalid: {e}"));
            set.add(name, file, true);
        }
        set
    }

    /// Built-in rules plus `*.toml` from the user rules folder.
    pub fn with_user_rules(dir: &Path) -> RuleSet {
        let mut set = RuleSet::builtin();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return set;
        };
        let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        files.sort();
        for p in files
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            match std::fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|t| toml::from_str::<RuleFile>(&t).map_err(|e| e.to_string()))
            {
                Ok(f) => set.add(&name, f, false),
                Err(e) => set.warnings.push(format!("{name}: {e}")),
            }
        }
        set
    }

    fn add(&mut self, name: &str, file: RuleFile, builtin: bool) {
        for p in &file.protect {
            self.protect.extend(p.paths.iter().cloned());
        }
        if builtin {
            self.ecosystems.extend(file.ecosystem.iter().cloned());
        }
        let Some(tool) = file.tool else { return };
        let mut rules = file.rule;
        if !builtin {
            // User rules can add, never loosen: no commands, no direct deletes, no Safe tier on
            // anything a built-in rule already covers, and ids may not shadow built-ins.
            let builtin_ids: BTreeSet<String> =
                self.all_rules().map(|(_, r)| r.id.clone()).collect();
            rules.retain(|r| {
                if builtin_ids.contains(&r.id) {
                    self.warnings.push(format!(
                        "{name}: rule {} shadows a built-in rule and was ignored",
                        r.id
                    ));
                    return false;
                }
                if r.disposal == DisposalDecl::Command {
                    self.warnings.push(format!(
                        "{name}: rule {} uses a command, which user rules can't",
                        r.id
                    ));
                    return false;
                }
                true
            });
            for r in &mut rules {
                if r.disposal == DisposalDecl::Delete {
                    r.disposal = DisposalDecl::Recycle;
                }
                r.detector.clear();
            }
        }
        match self.tools.iter_mut().find(|t| t.tool.id == tool.id) {
            Some(existing) => existing.rules.extend(rules),
            None => self.tools.push(LoadedTool {
                tool,
                rules,
                file: name.to_string(),
                builtin,
            }),
        }
    }

    pub fn all_rules(&self) -> impl Iterator<Item = (&LoadedTool, &RuleDef)> {
        self.tools
            .iter()
            .flat_map(|t| t.rules.iter().map(move |r| (t, r)))
    }

    pub fn rule(&self, id: &str) -> Option<(&LoadedTool, &RuleDef)> {
        self.all_rules().find(|(_, r)| r.id == id)
    }
}

/// Category for a tool's `category` string (rule ids can refine it).
pub fn category_for(tool: &ToolDef, rule: &RuleDef) -> CategoryId {
    if rule.id.starts_with("docker.") {
        return CategoryId::DevCaches;
    }
    match tool.category.as_str() {
        "ai-tools" => CategoryId::AiTools,
        "dev-caches" => CategoryId::DevCaches,
        "browsers" => CategoryId::Browsers,
        "downloads" => CategoryId::Downloads,
        "projects" => CategoryId::ProjectBuild,
        _ => CategoryId::Junk,
    }
}

/// Validation rules from docs/RULES.md, enforced by tests. Returns every problem found.
pub fn validate(set: &RuleSet) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids: BTreeMap<&str, &str> = BTreeMap::new();
    let mut tool_ids = BTreeSet::new();
    for t in &set.tools {
        if !tool_ids.insert(&t.tool.id) {
            errors.push(format!("{}: duplicate tool id {}", t.file, t.tool.id));
        }
        if !t
            .tool
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errors.push(format!(
                "{}: tool id {} must be kebab-case",
                t.file, t.tool.id
            ));
        }
        if ![
            "ai-tools",
            "dev-caches",
            "browsers",
            "system",
            "downloads",
            "projects",
        ]
        .contains(&t.tool.category.as_str())
        {
            errors.push(format!("{}: unknown category {}", t.file, t.tool.category));
        }
    }
    for (t, r) in set.all_rules() {
        if let Some(prev) = ids.insert(&r.id, &t.file) {
            errors.push(format!(
                "{}: duplicate rule id {} (also in {prev})",
                t.file, r.id
            ));
        }
        if !r.id.contains('.') {
            errors.push(format!(
                "{}: rule id {} must be <tool>.<slug>",
                t.file, r.id
            ));
        }
        if r.paths.is_empty() {
            errors.push(format!("{}: {} has no paths", t.file, r.id));
        }
        if r.disposal == DisposalDecl::Delete && r.comes_back == ComesBack::No {
            errors.push(format!(
                "{}: {} deletes directly but doesn't come back (use recycle)",
                t.file, r.id
            ));
        }
        if r.tier == TierDecl::Safe && (r.why.trim().is_empty() || r.if_removed.trim().is_empty()) {
            errors.push(format!(
                "{}: safe rule {} needs why and if_removed",
                t.file, r.id
            ));
        }
        if r.why.trim().is_empty() || r.if_removed.trim().is_empty() {
            errors.push(format!(
                "{}: {} must explain why and what you lose",
                t.file, r.id
            ));
        }
        if r.disposal == DisposalDecl::Command && r.command.is_empty() {
            errors.push(format!(
                "{}: {} has disposal = command but no command",
                t.file, r.id
            ));
        }
        if r.disposal == DisposalDecl::Guide && r.guide.is_empty() {
            errors.push(format!(
                "{}: {} has disposal = guide but no steps",
                t.file, r.id
            ));
        }
        if !r.detector.is_empty() && !engine::DETECTORS.contains(&r.detector.as_str()) {
            errors.push(format!(
                "{}: {} uses unknown detector {}",
                t.file, r.id, r.detector
            ));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_load_and_validate() {
        let set = RuleSet::builtin();
        assert!(set.tools.len() >= 10);
        assert!(set.all_rules().count() >= 30);
        assert!(!set.ecosystems.is_empty());
        let errors = validate(&set);
        assert!(errors.is_empty(), "{errors:#?}");
    }

    #[test]
    fn user_rules_cannot_loosen() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mine.toml"),
            r#"
[tool]
id = "my-tool"
name = "My tool"
category = "dev-caches"

[[rule]]
id = "my-tool.cache"
title = "Cache"
paths = ["~/.mytool/cache/**"]
tier = "safe"
disposal = "delete"
comes_back = "automatically"
why = "x"
if_removed = "y"

[[rule]]
id = "my-tool.run"
title = "Run"
paths = ["~/.mytool/x"]
tier = "safe"
disposal = "command"
command = ["calc.exe"]
comes_back = "automatically"
why = "x"
if_removed = "y"

[[rule]]
id = "npm.cache"
title = "Shadow"
paths = ["~/x"]
tier = "safe"
disposal = "delete"
comes_back = "automatically"
why = "x"
if_removed = "y"
"#,
        )
        .unwrap();
        let set = RuleSet::with_user_rules(dir.path());
        let (_, r) = set.rule("my-tool.cache").unwrap();
        assert_eq!(
            r.disposal,
            DisposalDecl::Recycle,
            "user rules never delete directly"
        );
        assert!(
            set.rule("my-tool.run").is_none(),
            "user rules can't run commands"
        );
        let (t, _) = set.rule("npm.cache").unwrap();
        assert!(t.builtin, "built-in rule wins");
        assert_eq!(set.warnings.len(), 2);
    }
}
