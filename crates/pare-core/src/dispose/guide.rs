//! Guide actions (M8): guide steps Pare can run for the user, always on an explicit click.
//! Docker/WSL disk compaction and Windows Update cleanup live here. None of them deletes
//! user files; each is still written to the operation log before it runs (invariant 8).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::executor::{new_op_id, run_command};
use super::oplog::{ItemStatus, LogRecord, OpLog};
use crate::env::Env;
use crate::i18n::{tr, trf};
use crate::platform::{self, now_ms};
use crate::rules::ACTION_PROGRAMS;
use crate::types::{Disposal, Finding, GuideAction};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionReport {
    pub op: String,
    pub ok: bool,
    pub output: String,
    /// Size on disk of the finding's paths before and after (compaction).
    pub before: u64,
    pub after: u64,
}

/// The `index`th action of a guide finding, if it is one Pare may run.
pub fn action_of(f: &Finding, index: usize) -> Result<&GuideAction, String> {
    let Disposal::Guide { actions, .. } = &f.disposal else {
        return Err(tr("This item has no steps Pare can run."));
    };
    let a = actions
        .get(index)
        .ok_or_else(|| tr("That step doesn't exist."))?;
    match a.run.first() {
        Some(p) if ACTION_PROGRAMS.contains(&p.as_str()) => Ok(a),
        _ => Err(tr("Pare doesn't run that command.")),
    }
}

/// Is `p` a virtual disk file Pare may compact?
fn is_vhd(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("vhdx") || e.eq_ignore_ascii_case("vhd"))
}

fn size_on_disk(paths: &[PathBuf]) -> u64 {
    paths
        .iter()
        .filter_map(|p| platform::stat(p).ok())
        .filter(|m| !m.is_dir)
        .map(|m| m.allocated)
        .sum()
}

/// Run a guide action. `elevated` is whether Pare runs as administrator; admin actions are
/// refused otherwise (the UI shows the command to run in an administrator terminal).
pub fn run_action(
    f: &Finding,
    index: usize,
    env: &Env,
    log: &OpLog,
    elevated: bool,
) -> Result<ActionReport, String> {
    let a = action_of(f, index)?.clone();
    if a.admin && !elevated {
        return Err(tr(
            "This step needs administrator rights. Restart Pare as administrator, or run the command in an administrator terminal.",
        ));
    }
    let compact = a.run[0] == "pare:compact-vhd";
    let disks: Vec<PathBuf> = if compact {
        f.paths.iter().filter(|p| is_vhd(p)).cloned().collect()
    } else {
        Vec::new()
    };
    if compact {
        if disks.is_empty() {
            return Err(tr("No virtual disk to compact."));
        }
        for d in &disks {
            let shown = d.display().to_string();
            let m = platform::stat(d)
                .map_err(|_| trf("{path} no longer exists.", &[("path", &shown)]))?;
            if m.is_reparse() || m.is_dir {
                return Err(trf(
                    "{path} isn't a virtual disk file.",
                    &[("path", &shown)],
                ));
            }
            if let Ok(h) = platform::processes_holding(std::slice::from_ref(d)) {
                if !h.is_empty() {
                    let names: Vec<String> = h.into_iter().map(|h| h.name).collect();
                    return Err(trf(
                        "The disk is in use by {apps}. Quit Docker Desktop and run 'wsl --shutdown' first.",
                        &[("apps", &names.join(", "))],
                    ));
                }
            }
        }
    }

    let started = now_ms();
    let op = new_op_id(started);
    let before = size_on_disk(if compact { &disks } else { &f.paths });
    let item = |result: ItemStatus, error: Option<String>, output: Option<String>, freed: u64| {
        LogRecord::Item {
            op: op.clone(),
            ts: now_ms(),
            item: f.id.clone(),
            title: format!("{}: {}", f.title, a.label),
            path: f
                .paths
                .first()
                .cloned()
                .or_else(|| f.source_path.clone())
                .unwrap_or_default(),
            paths: f.paths.clone(),
            bytes: 0,
            rule: f.rule_id.clone(),
            category: f.category,
            disposal: format!("guide-action:{}", a.run.join(" ")),
            result,
            error,
            freed,
            recycled: 0,
            output,
        }
    };
    log.write_all(&[
        LogRecord::OpBegin {
            op: op.clone(),
            ts: started,
            title: a.label.clone(),
            items: 1,
            bytes: 0,
        },
        item(ItemStatus::Pending, None, None, 0),
    ])
    .map_err(|e| {
        trf(
            "Couldn't write the operation log: {error}",
            &[("error", &e.to_string())],
        )
    })?;

    let (ok, output) = if compact {
        compact_all(&disks, env)
    } else {
        let (ok, code, out) = run_command(&a.run[0], &a.run[1..], Duration::from_secs(1800));
        let out = clean_output(&out);
        let out = if ok || code.is_none() {
            out
        } else {
            format!("Exited with code {}\n{out}", code.unwrap_or_default())
        };
        (ok, out)
    };
    let after = size_on_disk(if compact { &disks } else { &f.paths });
    let freed = before.saturating_sub(after);
    let _ = log.write_all(&[
        item(
            if ok {
                ItemStatus::Ok
            } else {
                ItemStatus::Failed
            },
            (!ok).then(|| output.lines().last().unwrap_or_default().to_string()),
            Some(output.clone()),
            freed,
        ),
        LogRecord::OpEnd {
            op: op.clone(),
            ts: now_ms(),
            freed_now: freed,
            recycled: 0,
            ok: ok as u64,
            failed: (!ok) as u64,
            skipped: 0,
        },
    ]);
    Ok(ActionReport {
        op,
        ok,
        output,
        before,
        after,
    })
}

/// wsl.exe writes UTF-16; drop the NULs so the text reads normally.
fn clean_output(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\0')
        .collect::<String>()
        .trim()
        .to_string()
}

/// Compact each disk with diskpart (part of Windows): attach read-only, compact, detach.
#[cfg(windows)]
fn compact_all(disks: &[PathBuf], env: &Env) -> (bool, String) {
    let mut all_ok = true;
    let mut out = String::new();
    for d in disks {
        let script = env.temp.join(format!("pare-compact-{:x}.txt", now_ms()));
        let path = d.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let body = format!(
            "select vdisk file=\"{path}\"\r\nattach vdisk readonly\r\ncompact vdisk\r\ndetach vdisk\r\nexit\r\n"
        );
        if let Err(e) = std::fs::write(&script, body) {
            return (false, format!("Couldn't write the diskpart script: {e}"));
        }
        let (ok, _, o) = run_command(
            "diskpart",
            &["/s".to_string(), script.to_string_lossy().into_owned()],
            Duration::from_secs(3600),
        );
        let _ = std::fs::remove_file(&script);
        all_ok &= ok;
        out.push_str(&clean_output(&o));
        out.push('\n');
    }
    (all_ok, out.trim().to_string())
}

#[cfg(not(windows))]
fn compact_all(_disks: &[PathBuf], _env: &Env) -> (bool, String) {
    (false, "Compacting virtual disks is Windows-only.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CategoryId, ComesBack, Dates, Tier};

    fn guide(actions: Vec<GuideAction>, paths: Vec<PathBuf>) -> Finding {
        Finding {
            id: "g".into(),
            rule_id: "docker.vhdx-guide".into(),
            category: CategoryId::DevCaches,
            source: "Docker".into(),
            source_path: None,
            title: "Docker disk".into(),
            paths,
            bytes: 0,
            logical_bytes: 0,
            items: 0,
            tier: Tier::Review,
            disposal: Disposal::Guide {
                steps: vec!["a".into(), "b".into()],
                actions,
            },
            comes_back: ComesBack::No,
            why: String::new(),
            if_removed: String::new(),
            dates: Dates::default(),
            created_by: None,
            badge: None,
            note: None,
            git: None,
            facts: Vec::new(),
        }
    }

    fn action(run: &[&str], admin: bool) -> GuideAction {
        GuideAction {
            step: 0,
            label: "x".into(),
            run: run.iter().map(|s| s.to_string()).collect(),
            admin,
        }
    }

    #[test]
    fn only_allowed_programs_run() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::fake(dir.path());
        let log = OpLog::open(&env.oplog_path()).unwrap();
        let f = guide(vec![action(&["cmd", "/c", "del", "x"], false)], Vec::new());
        assert!(run_action(&f, 0, &env, &log, true).is_err());
        assert!(run_action(&f, 5, &env, &log, true).is_err());
        // Nothing ran, so nothing was logged.
        assert!(super::super::oplog::read_log(&env.oplog_path()).is_empty());
    }

    #[test]
    fn admin_steps_refused_unless_elevated() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::fake(dir.path());
        let log = OpLog::open(&env.oplog_path()).unwrap();
        let disk = env.local_appdata.join("disk.vhdx");
        std::fs::write(&disk, b"not really a disk").unwrap();
        let f = guide(
            vec![action(&["pare:compact-vhd"], true)],
            vec![disk.clone()],
        );
        let e = run_action(&f, 0, &env, &log, false).unwrap_err();
        assert!(e.contains("administrator"), "{e}");
        assert!(super::super::oplog::read_log(&env.oplog_path()).is_empty());
        assert!(disk.exists());
    }

    #[test]
    fn compaction_only_touches_virtual_disks() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::fake(dir.path());
        let log = OpLog::open(&env.oplog_path()).unwrap();
        let other = env.local_appdata.join("notes.txt");
        std::fs::write(&other, b"keep").unwrap();
        let f = guide(
            vec![action(&["pare:compact-vhd"], true)],
            vec![other.clone()],
        );
        let e = run_action(&f, 0, &env, &log, true).unwrap_err();
        assert!(e.contains("No virtual disk"), "{e}");
        assert_eq!(std::fs::read(&other).unwrap(), b"keep");
    }

    #[test]
    fn utf16_output_reads_cleanly() {
        assert_eq!(clean_output("O\0K\0\r\n\0"), "OK");
    }
}
