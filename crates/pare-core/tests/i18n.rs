//! M8 localization: every string pare-core shows has a translation in every catalog, with the
//! same `{placeholders}`, and catalogs carry no stale entries.

use std::collections::BTreeSet;
use std::path::Path;

use pare_core::i18n::{LANGUAGES, catalog, placeholders, tr_in};
use pare_core::projects::ProjectStatus;
use pare_core::rules::RuleSet;
use pare_core::types::CategoryId;

/// String literals passed straight to `tr(` / `trf(` in the crate's sources.
fn literal_calls(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let rest = &src[i..];
        let hit = if rest.starts_with("tr(") {
            Some(3)
        } else if rest.starts_with("trf(") {
            Some(4)
        } else {
            None
        };
        let ident_before = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if let (Some(skip), false) = (hit, ident_before) {
            let mut j = i + skip;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let mut s = String::new();
                let mut k = j + 1;
                let chars: Vec<char> = src[k..].chars().collect();
                let mut c = 0;
                while c < chars.len() && chars[c] != '"' {
                    if chars[c] == '\\' && c + 1 < chars.len() {
                        match chars[c + 1] {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            other => s.push(other),
                        }
                        c += 2;
                    } else {
                        s.push(chars[c]);
                        c += 1;
                    }
                }
                k += chars[..c].iter().map(|ch| ch.len_utf8()).sum::<usize>();
                out.push(s);
                i = k + 1;
                continue;
            }
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    out
}

fn source_keys(dir: &Path, out: &mut BTreeSet<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            source_keys(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.extend(literal_calls(&std::fs::read_to_string(&p).unwrap()));
        }
    }
}

/// Every English string pare-core can show.
fn keys() -> BTreeSet<String> {
    let mut k = BTreeSet::new();
    let set = RuleSet::builtin();
    for t in &set.tools {
        k.insert(t.tool.name.clone());
        for r in &t.rules {
            k.insert(r.title.clone());
            k.insert(r.why.clone());
            k.insert(r.if_removed.clone());
            k.extend(r.guide.iter().cloned());
            k.extend(r.action.iter().map(|a| a.label.clone()));
        }
    }
    for c in CategoryId::ALL {
        // English is the default language in this test process.
        let info = c.info();
        k.insert(info.title);
        k.insert(info.description);
    }
    for s in ProjectStatus::ALL {
        k.insert(s.label().to_string());
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    source_keys(&here.join("src"), &mut k);
    // The desktop shell's notification area menu and notifications use the same catalog.
    source_keys(&here.join("../../apps/desktop/src-tauri/src"), &mut k);
    k.retain(|s| !s.trim().is_empty());
    k
}

#[test]
fn every_string_has_a_translation() {
    let keys = keys();
    assert!(keys.len() > 150, "only {} keys found", keys.len());
    for lang in LANGUAGES.iter().filter(|l| **l != "en") {
        let cat = catalog(lang);
        let missing: Vec<&String> = keys.iter().filter(|k| !cat.contains_key(*k)).collect();
        assert!(
            missing.is_empty(),
            "{lang}: {} strings have no translation:\n{}",
            missing.len(),
            missing
                .iter()
                .map(|k| format!("{} = \"\"", toml_key(k)))
                .collect::<Vec<_>>()
                .join("\n")
        );
        for k in &keys {
            let v = &cat[k];
            assert!(!v.trim().is_empty(), "{lang}: empty translation for {k:?}");
            assert_eq!(
                placeholders(k),
                placeholders(v),
                "{lang}: placeholders differ for {k:?} → {v:?}"
            );
        }
        let stale: Vec<&String> = cat.keys().filter(|k| !keys.contains(*k)).collect();
        assert!(stale.is_empty(), "{lang}: entries nothing uses: {stale:#?}");
    }
}

fn toml_key(s: &str) -> String {
    format!("{s:?}")
}

#[test]
fn spanish_samples() {
    assert_eq!(tr_in("es", "Recycle Bin"), "Papelera de reciclaje");
    assert_ne!(tr_in("es", "Junk & temp files"), "Junk & temp files");
}
