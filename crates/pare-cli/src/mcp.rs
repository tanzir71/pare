//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0). Every tool is read-only:
//! coding agents can ask what's using disk space and *propose* cleanups; executing one is
//! always left to the person (in the app, or with `pare clean --rule … --yes`).

use std::io::{BufRead, Write};
use std::path::PathBuf;

use pare_core::format::format_bytes;
use pare_core::scanner::ScanOutput;
use pare_core::{Env, Finding, Tier};
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";

fn tools() -> Value {
    json!([
        {
            "name": "disk_usage",
            "description": "Fixed drives with total and free space.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "scan_suggestions",
            "description": "Scan (read-only) and return Pare's cleanup suggestions: categories, sizes, and for each item why it's suggested, what you lose, whether it comes back, and its safety tier. Protected items are listed but can never be removed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Folders to scan. Default: every fixed drive." },
                    "max_items": { "type": "integer", "minimum": 1, "maximum": 200, "default": 25 },
                    "refresh": { "type": "boolean", "description": "Scan again instead of reusing the last result." }
                }
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "explain_path",
            "description": "Explain a file or folder: its size on disk, whether Pare would ever touch it, and which suggestion (if any) covers it.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "propose_cleanup",
            "description": "Build a cleanup proposal for rule ids or categories. Nothing is executed: the result lists the items and the exact command a person can run to do it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rule_ids": { "type": "array", "items": { "type": "string" } },
                    "categories": { "type": "array", "items": { "type": "string" } }
                }
            },
            "annotations": { "readOnlyHint": true }
        }
    ])
}

fn finding_json(f: &Finding) -> Value {
    json!({
        "id": f.id,
        "rule": f.rule_id,
        "category": f.category.as_str(),
        "title": f.title,
        "source": f.source,
        "path": f.paths.first(),
        "bytes": f.bytes,
        "size": format_bytes(f.bytes),
        "tier": match &f.tier { Tier::Safe => "safe".to_string(), Tier::Review => "review".to_string(), Tier::Protected { reason, .. } => format!("protected ({})", reason.explain()) },
        "disposal": f.disposal.kind(),
        "comes_back": f.comes_back.label(),
        "why": f.why,
        "if_removed": f.if_removed,
    })
}

struct Server {
    env: Env,
    last: Option<(Vec<PathBuf>, ScanOutput)>,
}

impl Server {
    fn scan(&mut self, paths: Vec<PathBuf>, refresh: bool) -> &ScanOutput {
        let stale = refresh || self.last.as_ref().is_none_or(|(p, _)| *p != paths);
        if stale {
            let out = crate::run_scan(&self.env, &paths, true);
            self.last = Some((paths, out));
        }
        &self.last.as_ref().expect("scan result").1
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "disk_usage" => Ok(json!(pare_core::platform::list_drives()
                .into_iter()
                .filter(|d| d.is_fixed)
                .map(|d| json!({ "drive": d.letter, "label": d.label, "total": format_bytes(d.total), "free": format_bytes(d.free), "used": format_bytes(d.used()) }))
                .collect::<Vec<_>>())),
            "scan_suggestions" => {
                let paths: Vec<PathBuf> = args
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).map(PathBuf::from).collect())
                    .unwrap_or_default();
                let max = args.get("max_items").and_then(|v| v.as_u64()).unwrap_or(25).clamp(1, 200) as usize;
                let refresh = args.get("refresh").and_then(|v| v.as_bool()).unwrap_or(false);
                let out = self.scan(paths, refresh);
                let mut findings: Vec<&Finding> = out.result.findings.iter().collect();
                findings.sort_by_key(|f| std::cmp::Reverse(f.bytes));
                Ok(json!({
                    "categories": out.result.categories.iter().map(|c| json!({
                        "id": c.info.id.as_str(), "title": c.info.title,
                        "safe": format_bytes(c.safe_bytes), "review": format_bytes(c.review_bytes), "items": c.items
                    })).collect::<Vec<_>>(),
                    "items": findings.iter().take(max).map(|f| finding_json(f)).collect::<Vec<_>>(),
                    "note": "Pare never deletes from MCP. Ask the person to review in the Pare app, or propose a command with propose_cleanup."
                }))
            }
            "explain_path" => {
                let path = PathBuf::from(args.get("path").and_then(|v| v.as_str()).ok_or("path is required")?);
                let m = pare_core::scan::measure::measure_path(&path);
                let never = pare_core::safety::protected_paths::classify(&path, &self.env, false);
                let covering: Vec<Value> = self
                    .last
                    .as_ref()
                    .map(|(_, o)| {
                        o.result
                            .findings
                            .iter()
                            .filter(|f| f.paths.iter().any(|p| pare_core::platform::path_starts_with(&path, p) || pare_core::platform::path_starts_with(p, &path)))
                            .map(finding_json)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(json!({
                    "path": path,
                    "size_on_disk": format_bytes(m.allocated),
                    "files": m.files,
                    "pare_never_touches": never.map(|r| r.explain()),
                    "suggestions": covering,
                    "hint": if covering.is_empty() { "Run scan_suggestions first to see whether a suggestion covers this path." } else { "" }
                }))
            }
            "propose_cleanup" => {
                let rules: Vec<String> = args.get("rule_ids").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let cats: Vec<String> = args.get("categories").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let out = self.scan(Vec::new(), false);
                let items: Vec<&Finding> = out
                    .result
                    .findings
                    .iter()
                    .filter(|f| f.is_selectable())
                    .filter(|f| rules.contains(&f.rule_id) || cats.iter().any(|c| c == f.category.as_str()))
                    .collect();
                let mut rule_ids: Vec<&str> = items.iter().map(|f| f.rule_id.as_str()).collect();
                rule_ids.sort();
                rule_ids.dedup();
                Ok(json!({
                    "items": items.iter().map(|f| finding_json(f)).collect::<Vec<_>>(),
                    "total": format_bytes(items.iter().map(|f| f.bytes).sum()),
                    "commands": rule_ids.iter().map(|r| format!("pare clean --rule {r} --yes")).collect::<Vec<_>>(),
                    "executed": false,
                    "note": "Nothing was removed. A person should review this (or open the Pare app) and run the commands themselves."
                }))
            }
            other => Err(format!("unknown tool {other}")),
        }
    }
}

pub fn serve(env: &Env) {
    let mut server = Server { env: env.clone(), last: None };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let Some(id) = id else { continue }; // notifications need no reply
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "pare", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Pare explains disk usage on Windows. All tools are read-only; never claim to have deleted anything."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match server.call(name, &args) {
                    Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }], "structuredContent": v }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                })
            }
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        let reply = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        };
        let _ = writeln!(stdout, "{reply}");
        let _ = stdout.flush();
    }
}
