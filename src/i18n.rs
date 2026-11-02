//! English / Spanish UI text, plus Papiamentu and Dutch (Beta) from a catalog
//! keyed by the English text (`translations.json`).
//!
//! The current language is a process-wide setting, applied from the user's
//! settings on every reload. Call sites keep both strings side by side:
//! `t("Loans", "Préstamos")`, or `tr!("Loan #{id}", "Préstamo #{id}")` for
//! formatted text, so a screen reads naturally in either language.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Es,
    Pap,
    Nl,
}

impl Lang {
    pub fn key(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
            Lang::Pap => "pap",
            Lang::Nl => "nl",
        }
    }
    pub fn from_key(k: &str) -> Self {
        match k {
            "es" => Lang::Es,
            "pap" => Lang::Pap,
            "nl" => Lang::Nl,
            _ => Lang::En,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::Pap => "Papiamentu (Beta)",
            Lang::Nl => "Nederlands (Beta)",
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn get() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Es,
        2 => Lang::Pap,
        3 => Lang::Nl,
        _ => Lang::En,
    }
}

pub fn is_es() -> bool {
    get() == Lang::Es
}

/// Pick the string for the current language.
pub fn t(en: &'static str, es: &'static str) -> &'static str {
    match get() {
        Lang::En => en,
        Lang::Es => es,
        lang => from_catalog(lang, en).unwrap_or(if lang == Lang::Pap { es } else { en }),
    }
}

/// The catalog: English text → (Papiamentu, Dutch).
fn catalog() -> &'static HashMap<String, (String, String)> {
    static CATALOG: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("translations.json")).expect("translations.json"))
}

fn from_catalog(lang: Lang, en: &str) -> Option<&'static str> {
    let (pap, nl) = catalog().get(en)?;
    Some(if lang == Lang::Pap { pap } else { nl })
}

/// The Papiamentu or Dutch text of a `tr!`, with the figures of the finished
/// English text carried across by placeholder name.
#[doc(hidden)]
pub fn fill(lang: Lang, template: &str, rendered: &str) -> Option<String> {
    let translated = from_catalog(lang, template)?;
    // Split the template into its fixed parts and placeholders, then find
    // each fixed part in what was rendered: what lies between is the value.
    let mut values: Vec<(&str, &str)> = Vec::new();
    let (mut rest, mut left) = (template, rendered);
    while let Some(open) = rest.find('{') {
        let close = open + rest[open..].find('}')?;
        let fixed = &rest[..open];
        left = left.strip_prefix(fixed)?;
        let name = &rest[open..=close];
        rest = &rest[close + 1..];
        let next = rest.find('{').map_or(rest, |i| &rest[..i]);
        let end = if next.is_empty() { if rest.is_empty() { left.len() } else { 0 } } else { left.find(next)? };
        values.push((name, &left[..end]));
        left = &left[end..];
    }
    let mut out = translated.to_string();
    for (name, value) in values {
        out = out.replace(name, value);
    }
    Some(out)
}

/// `format!` in the current language: `tr!("{n} loans", "{n} préstamos")`.
#[macro_export]
macro_rules! tr {
    ($en:literal, $es:literal $(,)?) => {
        match $crate::i18n::get() {
            $crate::i18n::Lang::Es => format!($es),
            $crate::i18n::Lang::En => format!($en),
            lang => {
                let en = format!($en);
                $crate::i18n::fill(lang, $en, &en).unwrap_or_else(|| if lang == $crate::i18n::Lang::Pap { format!($es) } else { en })
            }
        }
    };
}

/// "1 loan" / "2 loans" in the current language.
pub fn count(n: usize, en_one: &str, en_many: &str, es_one: &str, es_many: &str) -> String {
    let (en, es) = if n == 1 { (en_one, es_one) } else { (en_many, es_many) };
    let word = match get() {
        Lang::En => en,
        Lang::Es => es,
        lang => from_catalog(lang, en).unwrap_or(if lang == Lang::Pap { es } else { en }),
    };
    format!("{n} {word}")
}

const MONTHS_EN: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_PAP: [&str; 12] =
    ["yanüari", "febrüari", "mart", "aprel", "mei", "yüni", "yüli", "ougùstùs", "sèptèmber", "òktober", "novèmber", "desèmber"];
const MONTHS_NL: [&str; 12] =
    ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"];
const MONTHS_ES: [&str; 12] =
    ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const WEEKDAYS_EN: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const WEEKDAYS_ES: [&str; 7] = ["Do", "Lu", "Ma", "Mi", "Ju", "Vi", "Sá"];

/// Full month name, 1-based.
pub fn month_name(m: u32) -> &'static str {
    let i = (m.clamp(1, 12) - 1) as usize;
    match get() {
        Lang::En => MONTHS_EN[i],
        Lang::Es => MONTHS_ES[i],
        Lang::Pap => MONTHS_PAP[i],
        Lang::Nl => MONTHS_NL[i],
    }
}

/// Three-letter month: "Oct" / "oct".
pub fn month_short(m: u32) -> String {
    month_name(m).chars().take(3).collect()
}

/// Weekday abbreviations starting on Sunday.
pub fn weekdays() -> [&'static str; 7] {
    if is_es() { WEEKDAYS_ES } else { WEEKDAYS_EN }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_language() {
        set(Lang::Es);
        assert_eq!(t("Loans", "Préstamos"), "Préstamos");
        let n = 3;
        assert_eq!(tr!("{n} loans", "{n} préstamos"), "3 préstamos");
        assert_eq!(month_short(10), "oct");
        set(Lang::En);
        assert_eq!(month_short(10), "Oct");
        assert_eq!(count(1, "loan", "loans", "préstamo", "préstamos"), "1 loan");
    }

    #[test]
    fn figures_carry_across_into_the_catalog_text() {
        assert_eq!(
            fill(Lang::Pap, "Partly paid · {left} left", "Partly paid · $5.00 left").as_deref(),
            Some("Pagá parsialmente · falta $5.00")
        );
        assert_eq!(fill(Lang::Nl, "{a} of {b}", "$1 of $2").as_deref(), Some("$1 van $2"));
        assert_eq!(fill(Lang::Nl, "Not in the catalog {a}", "Not in the catalog 1"), None);
    }

    #[test]
    fn languages_survive_being_saved() {
        for lang in [Lang::En, Lang::Es, Lang::Pap, Lang::Nl] {
            assert_eq!(Lang::from_key(lang.key()), lang);
        }
    }

    #[test]
    fn every_catalog_entry_keeps_its_placeholders() {
        let names = |s: &str| {
            let mut found: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(n, _)| n.to_string())).collect();
            found.sort();
            found
        };
        for (en, (pap, nl)) in catalog() {
            assert_eq!(names(en), names(pap), "Papiamentu: {en}");
            assert_eq!(names(en), names(nl), "Dutch: {en}");
        }
    }

    /// Every text written as `t("…", …)` or `tr!("…", …)` is in the catalog, so
    /// Papiamentu and Dutch never fall back without anyone noticing.
    #[test]
    fn the_catalog_covers_the_apps_text() {
        fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("tests.rs") {
                    files.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut missing = Vec::new();
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            let source = source.split("#[cfg(test)]").next().unwrap();
            for opener in ["t(\"", "tr!(\""] {
                for (at, _) in source.match_indices(opener) {
                    if source[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let text = &source[at + opener.len()..];
                    let Some(end) = text.find("\",") else { continue };
                    let text = text[..end].replace("\\\"", "\"");
                    if !text.contains('\n') && !catalog().contains_key(&text) {
                        missing.push(text);
                    }
                }
            }
        }
        assert!(missing.is_empty(), "not in translations.json: {missing:#?}");
    }
}
