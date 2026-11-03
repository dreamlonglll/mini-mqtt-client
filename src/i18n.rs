//! Interface language (Simplified Chinese / English).
//!
//! Texts live in `locales/app.yml` and are looked up with `rust_i18n::t!`.
//! The locale is global and shared with GPUI Kit's components (they use
//! rust-i18n too), so [`apply`] switches both.

use serde::{Deserialize, Serialize};

/// rust-i18n locale codes (the same ones GPUI Kit's components use).
pub const ZH_CN: &str = "zh-CN";
pub const EN: &str = "en";

/// Language preference, stored in `config.yaml` as `language`. The values
/// are the ones the previous version kept in its `mqtt-client-locale`
/// setting: `auto` | `zh-CN` | `en-US`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Language {
    /// Follow the operating system (Chinese for any `zh*` locale, else English).
    #[default]
    System,
    Chinese,
    English,
}

impl Language {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "system" => Some(Language::System),
            "zh-cn" | "zh" | "zh_cn" => Some(Language::Chinese),
            "en-us" | "en" | "en_us" => Some(Language::English),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Language::System => "auto",
            Language::Chinese => "zh-CN",
            Language::English => "en-US",
        }
    }

    /// The concrete language this preference stands for right now.
    pub fn resolve(self) -> Language {
        match self {
            Language::System => system_language(),
            other => other,
        }
    }

    pub fn locale(self) -> &'static str {
        match self.resolve() {
            Language::Chinese => ZH_CN,
            _ => EN,
        }
    }
}

/// Unknown values (e.g. from a newer version) fall back to following the
/// system instead of making the whole config file unreadable.
impl From<String> for Language {
    fn from(value: String) -> Self {
        Language::parse(&value).unwrap_or_default()
    }
}

impl From<Language> for String {
    fn from(value: Language) -> Self {
        value.as_str().to_string()
    }
}

/// Language of a locale name such as `zh-CN`, `zh_TW.UTF-8` or `en-US`.
pub fn language_of_locale(locale: &str) -> Language {
    if locale.trim().to_ascii_lowercase().starts_with("zh") {
        Language::Chinese
    } else {
        Language::English
    }
}

pub fn system_language() -> Language {
    sys_locale::get_locale()
        .map(|l| language_of_locale(&l))
        .unwrap_or(Language::English)
}

/// `MINI_MQTT_LANG=zh-CN|en-US|auto` overrides the configured language at
/// start-up (used by the end-to-end tests, handy for screenshots).
pub fn env_override() -> Option<Language> {
    std::env::var("MINI_MQTT_LANG")
        .ok()
        .as_deref()
        .and_then(Language::parse)
}

/// Switch the interface (and GPUI Kit's components) to `language`.
pub fn apply(language: Language) {
    rust_i18n::set_locale(language.locale());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    const LOCALES: &str = include_str!("../locales/app.yml");

    /// `key.path -> {locale -> text}` of the locale file.
    fn entries() -> Vec<(String, serde_yaml::Mapping)> {
        fn walk(prefix: &str, value: &serde_yaml::Value, out: &mut Vec<(String, serde_yaml::Mapping)>) {
            let map = value.as_mapping().expect("mapping");
            let is_leaf = map.keys().any(|k| matches!(k.as_str(), Some(ZH_CN) | Some(EN)));
            if is_leaf {
                out.push((prefix.to_string(), map.clone()));
                return;
            }
            for (k, v) in map {
                let k = k.as_str().expect("string key");
                if k == "_version" {
                    continue;
                }
                let path = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{prefix}.{k}")
                };
                walk(&path, v, out);
            }
        }
        let root: serde_yaml::Value = serde_yaml::from_str(LOCALES).expect("locales/app.yml is valid YAML");
        let mut out = Vec::new();
        walk("", &root, &mut out);
        out
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        text.split("%{")
            .skip(1)
            .filter_map(|s| s.split_once('}'))
            .map(|(n, _)| n.to_string())
            .collect()
    }

    #[test]
    fn every_text_has_both_languages_with_the_same_placeholders() {
        let entries = entries();
        assert!(entries.len() > 100);
        for (key, map) in entries {
            let zh = map
                .get(ZH_CN)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("{key}: zh-CN missing"));
            let en = map
                .get(EN)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("{key}: en missing"));
            assert!(!zh.is_empty() && !en.is_empty(), "{key}: empty text");
            assert_eq!(placeholders(zh), placeholders(en), "{key}: placeholders differ");
            assert_eq!(map.len(), 2, "{key}: unexpected locales");
        }
    }

    /// Every literal key passed to the `t!` macro in the sources exists in
    /// the locale file.
    #[test]
    fn every_used_key_exists() {
        let known: BTreeSet<String> = entries().into_iter().map(|(k, _)| k).collect();
        let mut missing = Vec::new();
        let mut used = 0;
        fn scan(dir: &Path, f: &mut dyn FnMut(&Path, &str)) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path, f);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    f(&path, &std::fs::read_to_string(&path).unwrap());
                }
            }
        }
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        scan(&src, &mut |path, text| {
            for pattern in ["t!(\"", "tr!(\""] {
                for (at, _) in text.match_indices(pattern) {
                    // A macro name of its own, not the end of e.g. `format!(`.
                    let before = text[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let key = text[at + pattern.len()..].split('"').next().unwrap_or_default();
                    used += 1;
                    if !known.contains(key) {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        });
        assert!(used > 100, "scanner found only {used} keys");
        assert!(
            missing.is_empty(),
            "keys missing in locales/app.yml:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn translations_resolve_per_locale() {
        assert_eq!(rust_i18n::t!("common.cancel", locale = ZH_CN), "取消");
        assert_eq!(rust_i18n::t!("common.cancel", locale = EN), "Cancel");
        assert_eq!(
            rust_i18n::t!("toast.connected_to", locale = EN, name = "Local"),
            "Connected to Local"
        );
    }

    #[test]
    fn language_preference_parsing() {
        assert_eq!(Language::parse("auto"), Some(Language::System));
        assert_eq!(Language::parse("zh-CN"), Some(Language::Chinese));
        assert_eq!(Language::parse("en-US"), Some(Language::English));
        assert_eq!(Language::parse("fr"), None);
        assert_eq!(Language::from("bogus".to_string()), Language::System);
        assert_eq!(language_of_locale("zh-TW"), Language::Chinese);
        assert_eq!(language_of_locale("zh_CN.UTF-8"), Language::Chinese);
        assert_eq!(language_of_locale("en-GB"), Language::English);
        assert_eq!(language_of_locale("C"), Language::English);
        assert_eq!(Language::Chinese.locale(), ZH_CN);
        assert_eq!(Language::English.locale(), EN);
        assert!(matches!(
            Language::System.resolve(),
            Language::Chinese | Language::English
        ));
    }
}
