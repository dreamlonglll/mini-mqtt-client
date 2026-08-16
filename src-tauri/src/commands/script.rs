use tauri::State;
use crate::db::models::{CreateScriptRequest, UpdateScriptRequest};
use crate::db::Storage;

/// 获取服务器的所有脚本
#[tauri::command]
pub async fn list_scripts(storage: State<'_, Storage>, server_id: i64) -> Result<serde_json::Value, String> {
    storage.get_scripts_json(server_id)
}

/// 获取启用的脚本（按类型）
#[tauri::command]
pub async fn get_enabled_scripts(
    storage: State<'_, Storage>,
    server_id: i64,
    script_type: String,
) -> Result<serde_json::Value, String> {
    storage.get_enabled_scripts_json(server_id, &script_type)
}

/// 创建脚本
#[tauri::command]
pub async fn create_script(storage: State<'_, Storage>, request: CreateScriptRequest) -> Result<i64, String> {
    storage.create_script(request)
}

/// 更新脚本
#[tauri::command]
pub async fn update_script(storage: State<'_, Storage>, request: UpdateScriptRequest) -> Result<(), String> {
    storage.update_script(request)
}

/// 删除脚本
#[tauri::command]
pub async fn delete_script(storage: State<'_, Storage>, id: i64) -> Result<(), String> {
    storage.delete_script(id)
}

/// 切换脚本启用状态
#[tauri::command]
pub async fn toggle_script(storage: State<'_, Storage>, id: i64, enabled: bool) -> Result<(), String> {
    storage.toggle_script(id, enabled)
}
