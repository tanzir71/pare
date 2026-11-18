//! Translations for text pare-core produces: rule descriptions, card titles, detector
//! explanations and cleanup messages (M8 localization).
//!
//! Keys are the English text itself (gettext style), so English needs no catalog and a
//! missing translation falls back to English. Catalogs live in `crates/pare-core/locales/`;
//! a test checks every rule text and every `tr`/`trf` call has an entry in each catalog.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Languages with a catalog, besides English (index 0).
pub const LANGUAGES: &[&str] = &["en", "es"];

const CATALOGS: &[(&str, &str)] = &[("es", include_str!("../locales/es.toml"))];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// `es-MX` → `es`; anything without a catalog → `en`.
pub fn resolve(tag: &str) -> &'static str {
    let base = tag
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|l| **l == base)
        .copied()
        .unwrap_or("en")
}

/// Switch the language for text produced from now on (the next scan).
pub fn set_language(tag: &str) {
    let lang = resolve(tag);
    let i = LANGUAGES.iter().position(|l| *l == lang).unwrap_or(0);
    CURRENT.store(i, Ordering::Relaxed);
}

pub fn language() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed).min(LANGUAGES.len() - 1)]
}

/// The parsed catalog for `lang` (empty for English or an unknown language).
pub fn catalog(lang: &str) -> &'static HashMap<String, String> {
    static PARSED: OnceLock<Vec<(&'static str, HashMap<String, String>)>> = OnceLock::new();
    static EMPTY: OnceLock<HashMap<String, String>> = OnceLock::new();
    let all = PARSED.get_or_init(|| CATALOGS.iter().map(|(l, text)| (*l, parse(text))).collect());
    all.iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, m)| m)
        .unwrap_or_else(|| EMPTY.get_or_init(HashMap::new))
}

/// Catalog files are TOML with one `[strings]` table: `"English" = "Translation"`.
fn parse(text: &str) -> HashMap<String, String> {
    #[derive(serde::Deserialize)]
    struct File {
        strings: HashMap<String, String>,
    }
    toml::from_str::<File>(text)
        .map(|f| f.strings)
        .unwrap_or_else(|e| panic!("invalid locale catalog: {e}"))
}

/// Translate `en` into the current language (English when there's no entry).
pub fn tr(en: &str) -> String {
    tr_in(language(), en)
}

pub fn tr_in(lang: &str, en: &str) -> String {
    if lang == "en" {
        return en.to_string();
    }
    catalog(lang)
        .get(en)
        .cloned()
        .unwrap_or_else(|| en.to_string())
}

/// Translate a template, then fill `{name}` placeholders.
pub fn trf(en: &str, args: &[(&str, &str)]) -> String {
    fill(&tr(en), args)
}

pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut s = template.to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Placeholder names in a template, sorted (`{n}`, `{name}`).
pub fn placeholders(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + 1..start + end];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            out.push(name.to_string());
        }
        rest = &rest[start + end + 1..];
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_and_falls_back() {
        assert_eq!(resolve("es-MX"), "es");
        assert_eq!(resolve("ES"), "es");
        assert_eq!(resolve("fr-FR"), "en");
        assert_eq!(resolve(""), "en");
        assert_eq!(tr_in("en", "Recycle Bin"), "Recycle Bin");
        assert_eq!(tr_in("es", "no such string"), "no such string");
        assert_eq!(
            fill("Old CLI releases ({n})", &[("n", "4")]),
            "Old CLI releases (4)"
        );
        assert_eq!(placeholders("{b} grew {a} in {a}"), vec!["a", "a", "b"]);
    }
}
