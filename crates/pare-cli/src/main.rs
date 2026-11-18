//! `pare`: the command-line front end to pare-core.
//!
//! `pare scan` and `pare rules check` only read. `pare clean` is a dry run unless `--yes` is
//! given, follows the same safety pass and op log as the app, and never touches Protected items.

mod mcp;
mod rules_check;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand};
use pare_core::dispose::{CleanOptions, OpLog, clean};
use pare_core::format::{format_bytes, format_count};
use pare_core::rules::RuleSet;
use pare_core::scan::progress::Progress;
use pare_core::scanner::{ScanConfig, ScanOutput, ScanRequest, ScanRoot, scan};
use pare_core::settings::Settings;
use pare_core::{Env, Finding, Tier};

#[derive(Parser)]
#[command(
    name = "pare",
    version,
    about = "Make room on your PC. Keep what matters."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan and list suggestions (read-only).
    Scan {
        /// Folders to scan. Default: every fixed drive ("This PC").
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Skip the Restart Manager in-use check (faster).
        #[arg(long)]
        no_in_use: bool,
    },
    /// Remove what one rule (or category) suggests. Dry run unless --yes.
    Clean {
        /// Rule id (e.g. claude-code.debug-logs) or a prefix (claude-code.).
        #[arg(long)]
        rule: String,
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        /// Show what would happen without changing anything (the default).
        #[arg(long)]
        dry_run: bool,
        /// Actually do it (Safe and Review items only; Protected is never touched).
        #[arg(long)]
        yes: bool,
    },
    /// Cleanup rules.
    Rules {
        #[command(subcommand)]
        cmd: RulesCmd,
    },
    /// Run as an MCP server on stdio, so coding agents can ask what's using disk space and
    /// propose (never execute) cleanups.
    Mcp,
    /// Fast-mode index of an NTFS drive (needs an administrator terminal).
    Index {
        /// Drive root, e.g. C:\
        drive: PathBuf,
        /// Also save it to %LOCALAPPDATA%\Pare\index and time a USN catch-up.
        #[arg(long)]
        save: bool,
    },
}

#[derive(Subcommand)]
enum RulesCmd {
    /// List every rule.
    List,
    /// Check each rule's paths on this machine: list and measure, never delete.
    Check {
        /// Only rules whose id starts with this.
        #[arg(long)]
        only: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn rules(env: &Env) -> Arc<RuleSet> {
    Arc::new(RuleSet::with_user_rules(&env.user_rules_dir()))
}

pub(crate) fn run_scan(env: &Env, paths: &[PathBuf], in_use: bool) -> ScanOutput {
    let settings = Settings::load(env);
    let drives = pare_core::platform::list_drives();
    let req = if paths.is_empty() {
        let fixed: Vec<_> = drives.iter().filter(|d| d.is_fixed).collect();
        ScanRequest {
            roots: fixed
                .iter()
                .map(|d| ScanRoot {
                    path: d.root.clone(),
                    label: format!("This PC ({})", d.letter),
                })
                .collect(),
            drives: fixed.iter().map(|d| d.root.clone()).collect(),
            whole_drives: true,
        }
    } else {
        ScanRequest {
            roots: paths
                .iter()
                .map(|p| ScanRoot {
                    path: p.clone(),
                    label: p.to_string_lossy().into_owned(),
                })
                .collect(),
            drives: Vec::new(),
            whole_drives: false,
        }
    };
    let cfg = ScanConfig {
        thresholds: settings.thresholds.clone(),
        exclusions: settings.exclusions.clone(),
        check_in_use: in_use,
        include_recycle_bin: true,
        now: None,
        rules: Some(rules(env)),
        project_roots: pare_core::projects::default_roots(env)
            .into_iter()
            .chain(settings.project_roots.iter().cloned())
            .collect(),
        duplicates: true,
        fast_mode: true,
        index_cache: None,
    };
    scan(
        &req,
        &cfg,
        env,
        &Progress::default(),
        &AtomicBool::new(false),
        &|_| {},
    )
}

fn tier_label(t: &Tier) -> String {
    match t {
        Tier::Safe => "Safe".into(),
        Tier::Review => "Review".into(),
        Tier::Protected { reason, .. } => format!("Kept ({})", reason.badge()),
    }
}

fn print_findings(findings: &[&Finding]) {
    for f in findings {
        println!(
            "  {:>9}  {:<12} {:<44} {}",
            format_bytes(f.bytes),
            tier_label(&f.tier),
            f.title.chars().take(44).collect::<String>(),
            f.paths
                .first()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
    }
}

fn main() {
    let cli = Cli::parse();
    let env = Env::from_process();
    match cli.cmd {
        Cmd::Scan {
            paths,
            json,
            no_in_use,
        } => {
            let out = run_scan(&env, &paths, !no_in_use);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out.result).unwrap_or_default()
                );
                return;
            }
            let r = &out.result;
            println!(
                "Scanned {} files in {:.1} s\n",
                format_count(r.stats.files),
                r.stats.elapsed_ms as f64 / 1000.0
            );
            for (name, ms) in &r.stats.phases {
                eprintln!("  {name:<32} {ms:>8} ms");
            }
            for c in &r.categories {
                println!(
                    "{} — {} ({} safe · {} to review)",
                    c.info.title,
                    format_bytes(c.total_bytes()),
                    format_bytes(c.safe_bytes),
                    format_bytes(c.review_bytes)
                );
                let mut fs: Vec<&Finding> = r
                    .findings
                    .iter()
                    .filter(|f| f.category == c.info.id)
                    .collect();
                fs.sort_by_key(|f| std::cmp::Reverse(f.bytes));
                print_findings(&fs.into_iter().take(8).collect::<Vec<_>>());
                println!();
            }
        }
        Cmd::Clean {
            rule,
            paths,
            dry_run,
            yes,
        } => {
            let out = run_scan(&env, &paths, true);
            let chosen: Vec<Finding> = out
                .result
                .findings
                .iter()
                .filter(|f| {
                    f.rule_id == rule || (rule.ends_with('.') && f.rule_id.starts_with(&rule))
                })
                .cloned()
                .collect();
            let selectable: Vec<Finding> = chosen
                .iter()
                .filter(|f| f.is_selectable())
                .cloned()
                .collect();
            println!(
                "{} matching items, {} selectable:",
                chosen.len(),
                selectable.len()
            );
            print_findings(&chosen.iter().collect::<Vec<_>>());
            let total: u64 = selectable.iter().map(|f| f.bytes).sum();
            if dry_run || !yes {
                println!(
                    "\nDry run: would free up {}. Add --yes to do it.",
                    format_bytes(total)
                );
                return;
            }
            let log = match OpLog::open(&env.oplog_path()) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Couldn't open the operation log: {e}");
                    std::process::exit(1);
                }
            };
            let report = clean(
                &selectable,
                &env,
                &log,
                &CleanOptions::new(out.result.started),
                &|_| {},
            );
            println!(
                "\nFreed {} now · {} to the Recycle Bin · {} done, {} failed, {} skipped",
                format_bytes(report.freed_now),
                format_bytes(report.recycled),
                report.ok,
                report.failed,
                report.skipped
            );
            if let Some(s) = report.summary {
                println!("{s}");
            }
        }
        Cmd::Rules {
            cmd: RulesCmd::List,
        } => {
            let set = rules(&env);
            for (t, r) in set.all_rules() {
                println!(
                    "{:<40} {:<24} {:<7} {:<8} {}{}",
                    r.id,
                    t.tool.name,
                    format!("{:?}", r.tier).to_lowercase(),
                    format!("{:?}", r.disposal).to_lowercase(),
                    r.title,
                    if r.verified { "" } else { "  [unverified]" }
                );
            }
            for w in &set.warnings {
                eprintln!("warning: {w}");
            }
        }
        Cmd::Rules {
            cmd: RulesCmd::Check { only, json },
        } => {
            let set = rules(&env);
            let reports = rules_check::check(&set, &env, only.as_deref());
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&reports).unwrap_or_default()
                );
            } else {
                rules_check::print(&reports);
            }
        }
        Cmd::Mcp => mcp::serve(&env),
        Cmd::Index { drive, save } => index_cmd(&env, &drive, save),
    }
}

#[cfg(windows)]
fn index_cmd(env: &Env, drive: &std::path::Path, save: bool) {
    use pare_core::index::{db_path, mft, store};
    if !pare_core::platform::is_elevated() {
        eprintln!("Fast mode reads the NTFS file table and needs an administrator terminal.");
        std::process::exit(2);
    }
    let t = std::time::Instant::now();
    let mut idx = match mft::build(drive) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Couldn't read the MFT: {e}");
            std::process::exit(1);
        }
    };
    let files = idx.entries.iter().filter(|e| !e.is_dir).count();
    println!(
        "{}: {} entries, {} files, read in {:.2} s",
        drive.display(),
        format_count(idx.entries.len() as u64),
        format_count(files as u64),
        t.elapsed().as_secs_f64()
    );
    if save {
        let p = db_path(env, idx.serial);
        let t = std::time::Instant::now();
        match store::save(&idx, &p) {
            Ok(()) => println!(
                "saved to {} in {:.2} s",
                p.display(),
                t.elapsed().as_secs_f64()
            ),
            Err(e) => eprintln!("couldn't save: {e}"),
        }
        let t = std::time::Instant::now();
        match mft::update(&mut idx) {
            Ok(n) => println!(
                "USN catch-up: {n} changes in {:.3} s",
                t.elapsed().as_secs_f64()
            ),
            Err(e) => eprintln!("journal: {e}"),
        }
    }
}

#[cfg(not(windows))]
fn index_cmd(_env: &Env, _drive: &std::path::Path, _save: bool) {
    eprintln!("Fast mode is Windows-only.");
    std::process::exit(2);
}
