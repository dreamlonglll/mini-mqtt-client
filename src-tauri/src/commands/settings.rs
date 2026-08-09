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

/// 迁移数据到新路径（async：文件复制/写盘在 tokio worker 上执行，不阻塞主线程）
#[tauri::command]
pub async fn migrate_data_path(
    app_handle: AppHandle,
    storage: tauri::State<'_, Storage>,
    new_path: String,
    migrate: bool,
) -> Result<(), String> {
    let new_path = PathBuf::from(&new_path);

    // 验证新路径
    if !new_path.is_absolute() {
        return Err("Please provide an absolute path".to_string());
    }

    // 确保目标目录存在
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // 如果需要迁移，复制当前数据到新位置
    if migrate {
        // 防抖落盘可能滞后，先把内存中的修改刷到磁盘再复制
        storage.flush();
        let current_path = storage.get_file_path();
        if current_path.exists() {
            fs::copy(&current_path, &new_path)
                .map_err(|e| format!("Failed to copy data file: {}", e))?;
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
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // 更新运行中的存储路径，后续落盘直接写入新位置（无需重启）
    storage.set_file_path(new_path);

    Ok(())
}

/// 选择文件夹对话框（spawn_blocking：阻塞式对话框不占用 tokio worker）
#[tauri::command]
pub async fn select_data_folder(app_handle: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = tauri::async_runtime::spawn_blocking(move || {
        app_handle
            .dialog()
            .file()
            .set_title("Select Data Directory")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;

    match folder {
        Some(file_path) => {
            // FilePath 需要转换为 PathBuf
            let path_buf = file_path.as_path().ok_or("Invalid path")?;
            let data_file = path_buf.join("data.yaml");
            Ok(Some(data_file.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}
