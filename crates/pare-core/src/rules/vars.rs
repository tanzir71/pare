//! Path variables: `~`, `%APPDATA%`, `%LOCALAPPDATA%`, `%TEMP%`, `%PROGRAMDATA%`,
//! `${ENV:-default}` (nestable) and `{drive}` (each fixed drive root). Output uses `/`.

use std::path::PathBuf;

use crate::env::Env;

/// Expand one TOML path pattern into concrete glob patterns (several when `{drive}` is used).
pub fn expand(pattern: &str, env: &Env, drives: &[PathBuf]) -> Vec<String> {
    let s = expand_env_defaults(pattern, env);
    let s = expand_percent(&s, env);
    let s = if s == "~" {
        env.home.to_string_lossy().into_owned()
    } else if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        format!("{}/{rest}", env.home.to_string_lossy())
    } else {
        s
    };
    let s = s.replace('\\', "/");
    if s.contains("{drive}") {
        drives
            .iter()
            .map(|d| {
                let root = d.to_string_lossy().replace('\\', "/");
                s.replace("{drive}/", &root).replace("{drive}", root.trim_end_matches('/'))
            })
            .collect()
    } else {
        vec![s]
    }
}

/// `${NAME:-default}` → the variable if set and non-empty, else the (expanded) default.
fn expand_env_defaults(s: &str, env: &Env) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let body_start = start + 2;
        // Find the matching close brace (defaults may nest ${…}).
        let mut depth = 1;
        let mut end = None;
        let bytes = rest.as_bytes();
        let mut i = body_start;
        while i < bytes.len() {
            if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
                depth += 1;
                i += 2;
                continue;
            }
            if bytes[i] == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            i += 1;
        }
        let Some(end) = end else {
            out.push_str(&rest[start..]);
            return out;
        };
        let body = &rest[body_start..end];
        let (name, default) = match body.find(":-") {
            Some(p) => (&body[..p], Some(&body[p + 2..])),
            None => (body, None),
        };
        let value = env.var(name).filter(|v| !v.is_empty());
        match (value, default) {
            (Some(v), _) => out.push_str(&v),
            (None, Some(d)) => {
                let d = expand_env_defaults(d, env);
                let d = expand_percent(&d, env);
                // A default may start with ~
                if let Some(r) = d.strip_prefix("~/").or_else(|| d.strip_prefix("~\\")) {
                    out.push_str(&format!("{}/{r}", env.home.to_string_lossy()));
                } else {
                    out.push_str(&d);
                }
            }
            (None, None) => {}
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `%NAME%` → the variable; unknown names are left as-is (and won't match anything).
fn expand_percent(s: &str, env: &Env) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        let Some(b) = rest[a + 1..].find('%') else { break };
        let name = &rest[a + 1..a + 1 + b];
        out.push_str(&rest[..a]);
        match env.var(name) {
            Some(v) if !name.is_empty() => out.push_str(&v),
            _ => out.push_str(&rest[a..a + b + 2]),
        }
        rest = &rest[a + b + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> (tempfile::TempDir, Env) {
        let d = tempfile::tempdir().unwrap();
        let e = Env::fake(d.path());
        (d, e)
    }

    #[test]
    fn tilde_and_percent() {
        let (_d, e) = env();
        let home = e.home.to_string_lossy().replace('\\', "/");
        assert_eq!(expand("~/.claude/debug/**", &e, &[]), vec![format!("{home}/.claude/debug/**")]);
        let local = e.local_appdata.to_string_lossy().replace('\\', "/");
        assert_eq!(expand("%LOCALAPPDATA%/npm-cache/**", &e, &[]), vec![format!("{local}/npm-cache/**")]);
        assert_eq!(expand("%NOPE%/x", &e, &[]), vec!["%NOPE%/x".to_string()]);
    }

    #[test]
    fn env_defaults_nest() {
        let (_d, e) = env();
        let home = e.home.to_string_lossy().replace('\\', "/");
        assert_eq!(
            expand("${OLLAMA_MODELS:-~/.ollama/models}/manifests/**", &e, &[]),
            vec![format!("{home}/.ollama/models/manifests/**")]
        );
        assert_eq!(
            expand("${HF_HUB_CACHE:-${HF_HOME:-~/.cache/huggingface}/hub}/models--*", &e, &[]),
            vec![format!("{home}/.cache/huggingface/hub/models--*")]
        );
        let e2 = e.clone().with_var("HF_HOME", "D:/hf");
        assert_eq!(expand("${HF_HUB_CACHE:-${HF_HOME:-~/.cache/huggingface}/hub}/models--*", &e2, &[]), vec!["D:/hf/hub/models--*".to_string()]);
        let e3 = e.with_var("GOCACHE", "E:\\go");
        assert_eq!(expand("${GOCACHE:-%LOCALAPPDATA%/go-build}/**", &e3, &[]), vec!["E:/go/**".to_string()]);
    }

    #[test]
    fn drives() {
        let (_d, e) = env();
        let v = expand("{drive}/WindowsApps/x", &e, &[PathBuf::from("C:\\"), PathBuf::from("D:\\")]);
        assert_eq!(v, vec!["C:/WindowsApps/x".to_string(), "D:/WindowsApps/x".to_string()]);
    }
}
