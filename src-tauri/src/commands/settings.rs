use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::db::Storage;

/// 获取当前数据存储路径
#[tauri::command]
pub fn get_data_path(storage: tauri::State<Storage>) -> Result<String, String> {
    Ok(storage.get_file_path().to_string_lossy().to_string())
}

/// 迁移数据到新路径
#[tauri::command]
pub fn migrate_data_path(
    app_handle: AppHandle,
    storage: tauri::State<Storage>,
    new_path: String,
    migrate: bool,
) -> Result<(), String> {
    let new_path = PathBuf::from(&new_path);
    
    // 验证新路径
    if !new_path.is_absolute() {
        return Err("请提供绝对路径".to_string());
    }
    
    // 确保目标目录存在
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建目录: {}", e))?;
    }
    
    // 如果需要迁移，复制当前数据到新位置
    if migrate {
        let current_path = storage.get_file_path();
        if current_path.exists() {
            fs::copy(current_path, &new_path)
                .map_err(|e| format!("复制数据文件失败: {}", e))?;
        }
    }
    
    // 保存新路径配置（使用单独的配置文件）
    let config_path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config.yaml");
    
    let mut config_map: HashMap<String, String> = HashMap::new();
    config_map.insert("data_path".to_string(), new_path.to_string_lossy().to_string());
    
    let config = serde_yaml::to_string(&config_map).map_err(|e| e.to_string())?;
    
    fs::write(&config_path, config)
        .map_err(|e| format!("保存配置失败: {}", e))?;
    
    Ok(())
}

/// 选择文件夹对话框
#[tauri::command]
pub async fn select_data_folder(app_handle: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let folder = app_handle
        .dialog()
        .file()
        .set_title("选择数据存储目录")
        .blocking_pick_folder();
    
    match folder {
        Some(file_path) => {
            // FilePath 需要转换为 PathBuf
            let path_buf = file_path.as_path().ok_or("无效的路径")?;
            let data_file = path_buf.join("data.yaml");
            Ok(Some(data_file.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}
