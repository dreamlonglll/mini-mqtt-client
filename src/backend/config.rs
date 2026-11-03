//! Application paths and the small `config.yaml` (custom data path, theme,
//! language, message limit).

use crate::i18n::Language;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Same identifier as the previous Tauri build, so the existing data
/// directory (and `data.yaml` in it) is picked up automatically.
pub const APP_IDENTIFIER: &str = "com.minimqtt.client";

/// Messages kept per server in the live list (the previous version's
/// "message list limit" setting: default 1000, 100–10000).
pub const DEFAULT_MESSAGE_LIMIT: usize = 1000;
pub const MIN_MESSAGE_LIMIT: usize = 100;
pub const MAX_MESSAGE_LIMIT: usize = 10_000;

fn default_message_limit() -> usize {
    DEFAULT_MESSAGE_LIMIT
}

/// Platform data directory of the application, e.g.
/// `%APPDATA%\com.minimqtt.client` on Windows.
pub fn app_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MINI_MQTT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_IDENTIFIER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

/// Unknown values fall back to the default instead of making the whole
/// config file (and with it the custom data path) unreadable.
impl From<String> for ThemePreference {
    fn from(value: String) -> Self {
        match value.as_str() {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        }
    }
}

impl From<ThemePreference> for String {
    fn from(value: ThemePreference) -> Self {
        match value {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_path: Option<String>,
    #[serde(default)]
    pub theme: ThemePreference,
    /// Interface language: `auto` (follow the system), `zh-CN` or `en-US`.
    #[serde(default)]
    pub language: Language,
    /// Messages kept per server (clamped to 100–10000 when used).
    #[serde(default = "default_message_limit")]
    pub message_limit: usize,
    /// Keys this version does not model, written back unchanged.
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_path: None,
            theme: ThemePreference::default(),
            language: Language::default(),
            message_limit: DEFAULT_MESSAGE_LIMIT,
            extra: serde_yaml::Mapping::new(),
        }
    }
}

impl AppConfig {
    fn path(app_dir: &Path) -> PathBuf {
        app_dir.join("config.yaml")
    }

    pub fn load(app_dir: &Path) -> Self {
        fs::read_to_string(Self::path(app_dir))
            .ok()
            .and_then(|content| serde_yaml::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app_dir: &Path) -> Result<(), String> {
        fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
        let content = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        super::storage::write_atomic(&Self::path(app_dir), content.as_bytes())
            .map_err(|e| tr!("config.save_failed", error = e).to_string())
    }

    /// Resolve the data file: the custom path if it (or its parent) exists,
    /// otherwise `<app_dir>/data.yaml`.
    pub fn data_file(&self, app_dir: &Path) -> PathBuf {
        if let Some(custom) = self.data_path.as_deref().filter(|p| !p.trim().is_empty()) {
            let custom = PathBuf::from(custom);
            if custom.exists() || custom.parent().map(|p| p.exists()).unwrap_or(false) {
                return custom;
            }
        }
        app_dir.join("data.yaml")
    }
}

/// Point the application at a new data file, optionally copying the current
/// data there first. Takes effect after restart (as before).
pub fn migrate_data_path(
    app_dir: &Path,
    config: &mut AppConfig,
    current_file: &Path,
    new_path: &Path,
    copy_data: bool,
) -> Result<(), String> {
    if !new_path.is_absolute() {
        return Err(tr!("config.absolute_path").to_string());
    }
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| tr!("config.create_dir_failed", error = e).to_string())?;
    }
    if copy_data && current_file.exists() && current_file != new_path {
        fs::copy(current_file, new_path).map_err(|e| tr!("config.copy_failed", error = e).to_string())?;
    }
    config.data_path = Some(new_path.to_string_lossy().to_string());
    config.save(app_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_with_only_data_path_loads() {
        let config: AppConfig = serde_yaml::from_str("data_path: /tmp/x/data.yaml\n").unwrap();
        assert_eq!(config.data_path.as_deref(), Some("/tmp/x/data.yaml"));
        assert_eq!(config.theme, ThemePreference::System);
        assert_eq!(config.language, Language::System);
        assert_eq!(config.message_limit, DEFAULT_MESSAGE_LIMIT);
        assert_eq!(AppConfig::default().message_limit, DEFAULT_MESSAGE_LIMIT);
    }

    #[test]
    fn language_and_theme_round_trip_and_tolerate_unknown_values() {
        let config: AppConfig = serde_yaml::from_str("theme: dark\nlanguage: en-US\n").unwrap();
        assert_eq!(config.theme, ThemePreference::Dark);
        assert_eq!(config.language, Language::English);
        let text = serde_yaml::to_string(&config).unwrap();
        assert!(
            text.contains("theme: dark") && text.contains("language: en-US"),
            "{text}"
        );
        let odd: AppConfig = serde_yaml::from_str("data_path: /x/data.yaml\ntheme: neon\nlanguage: fr\n").unwrap();
        assert_eq!(
            odd.data_path.as_deref(),
            Some("/x/data.yaml"),
            "data path survives odd values"
        );
        assert_eq!((odd.theme, odd.language), (ThemePreference::System, Language::System));
    }

    #[test]
    fn unknown_config_keys_survive_a_save() {
        let dir = std::env::temp_dir().join(format!("mini-mqtt-test-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.yaml"), "theme: dark\nfuture_option: 3\n").unwrap();
        let mut config = AppConfig::load(&dir);
        assert_eq!(config.theme, ThemePreference::Dark);
        config.theme = ThemePreference::Light;
        config.save(&dir).unwrap();
        let saved: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(dir.join("config.yaml")).unwrap()).unwrap();
        assert_eq!(saved["theme"], serde_yaml::Value::from("light"));
        assert_eq!(saved["future_option"], serde_yaml::Value::from(3));
        let _ = fs::remove_dir_all(dir);
    }
}
