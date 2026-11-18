//! Ollama models for the `ollama.models` rule: one finding per model tag, removed with
//! `ollama rm <model>`. Size counts only blobs no other model uses (shared layers stay).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform;
use crate::rules::engine::{Base, Walker};
use crate::rules::matcher::Match;
use crate::scan::measure::{Candidate, FileAcc, Measured, account};
use crate::types::{Dates, Disposal, MS_PER_DAY};

struct Manifest {
    path: PathBuf,
    name: String,
    modified: i64,
    created: i64,
    digests: Vec<String>,
}

/// `manifests/registry.ollama.ai/library/llama3/8b` → `llama3:8b`; other registries keep
/// their prefix (`hf.co/user/model:tag`).
pub fn model_name(manifests_dir: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(manifests_dir).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let tag = parts.last()?.clone();
    let model = parts[parts.len() - 2].clone();
    let prefix = &parts[..parts.len() - 2];
    let name = match prefix {
        [reg, ns] if reg == "registry.ollama.ai" && ns == "library" => model,
        _ => format!("{}/{}", prefix.join("/"), model),
    };
    Some(format!("{name}:{tag}"))
}

fn files_under(dir: &Path, out: &mut Vec<(PathBuf, platform::RawEntry)>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(children) = platform::list_dir(dir) else {
        return;
    };
    for c in children {
        let p = dir.join(&c.name);
        if c.is_traversable_dir() {
            files_under(&p, out, depth + 1);
        } else if !c.is_dir {
            out.push((p, c));
        }
    }
}

pub fn models(w: &Walker, base: &Base, matches: &[Match]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for m in matches {
        let Match::Contents(manifests_dir) = m else {
            continue;
        };
        let Some(models_dir) = manifests_dir.parent() else {
            continue;
        };
        let mut files = Vec::new();
        files_under(manifests_dir, &mut files, 0);
        let mut manifests: Vec<Manifest> = Vec::new();
        for (p, e) in files {
            if e.is_cloud_placeholder() || e.logical > 1_000_000 {
                continue;
            }
            let Some(name) = model_name(manifests_dir, &p) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let mut digests = Vec::new();
            if let Some(d) = v.pointer("/config/digest").and_then(|d| d.as_str()) {
                digests.push(d.to_string());
            }
            if let Some(layers) = v.get("layers").and_then(|l| l.as_array()) {
                for l in layers {
                    if let Some(d) = l.get("digest").and_then(|d| d.as_str()) {
                        digests.push(d.to_string());
                    }
                }
            }
            manifests.push(Manifest {
                path: p,
                name,
                modified: e.modified,
                created: e.created,
                digests,
            });
        }
        let mut uses: HashMap<&str, u32> = HashMap::new();
        for mf in &manifests {
            for d in &mf.digests {
                *uses.entry(d.as_str()).or_default() += 1;
            }
        }
        let min_age = w.rule.min_age_days.unwrap_or(0) as i64 * MS_PER_DAY;
        for mf in &manifests {
            if mf.modified > w.ctx.now - min_age {
                continue;
            }
            let mut accs: Vec<FileAcc> = mf
                .digests
                .iter()
                .filter(|d| uses.get(d.as_str()) == Some(&1))
                .filter_map(|d| {
                    let blob = models_dir.join("blobs").join(d.replace(':', "-"));
                    platform::stat(&blob).ok().map(|e| FileAcc {
                        path: blob,
                        allocated: e.allocated,
                        logical: e.logical,
                        file_id: e.file_id,
                        links: e.links,
                        modified: e.modified,
                    })
                })
                .collect();
            let (bytes, logical, _) = account(&mut accs);
            let mut cand = Candidate::new(&w.rule.id, base.category, format!("Model {}", mf.name));
            cand.source = base.tool.tool.name.clone();
            cand.source_path = Some(models_dir.to_path_buf());
            cand.created_by = Some("Ollama".into());
            cand.extra_paths = vec![mf.path.clone()];
            cand.tier = crate::types::Tier::Review;
            cand.disposal = Disposal::Command {
                program: "ollama".into(),
                args: vec!["rm".into(), mf.name.clone()],
            };
            cand.comes_back = w.rule.comes_back;
            cand.why = w.rule.why.clone();
            cand.if_removed = w.rule.if_removed.clone();
            cand.note = Some(format!(
                "Pulled {}",
                crate::format::age_phrase(w.ctx.now, mf.modified)
            ));
            cand.facts.push(("Model".into(), mf.name.clone()));
            cand.facts.push(("Rule".into(), w.rule.id.clone()));
            let mut dates = Dates::default();
            dates.include(mf.created, mf.modified, 0);
            cand.measured = Some(Measured {
                bytes,
                logical,
                items: accs.len() as u64,
                dates,
                newest: mf.modified,
                recent_files: Vec::new(),
            });
            out.push(cand);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let m = Path::new("/x/models/manifests");
        assert_eq!(
            model_name(m, &m.join("registry.ollama.ai/library/llama3/8b")).as_deref(),
            Some("llama3:8b")
        );
        assert_eq!(
            model_name(m, &m.join("hf.co/user/model/q4")).as_deref(),
            Some("hf.co/user/model:q4")
        );
        assert_eq!(
            model_name(m, &m.join("registry.ollama.ai/me/tiny/latest")).as_deref(),
            Some("registry.ollama.ai/me/tiny:latest")
        );
    }
}
