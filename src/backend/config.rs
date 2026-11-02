//! Application paths and the small `config.yaml` (custom data path, theme).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Same identifier as the previous Tauri build, so the existing data
/// directory (and `data.yaml` in it) is picked up automatically.
pub const APP_IDENTIFIER: &str = "com.minimqtt.client";

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
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_path: Option<String>,
    #[serde(default)]
    pub theme: ThemePreference,
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
        fs::write(Self::path(app_dir), content).map_err(|e| format!("保存配置失败: {e}"))
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
        return Err("请提供绝对路径".to_string());
    }
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建目录: {e}"))?;
    }
    if copy_data && current_file.exists() && current_file != new_path {
        fs::copy(current_file, new_path).map_err(|e| format!("复制数据文件失败: {e}"))?;
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
    }
}
