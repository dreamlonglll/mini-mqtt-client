use tauri::State;
use crate::db::models::{Script, CreateScriptRequest, UpdateScriptRequest};
use crate::db::Storage;

/// 获取服务器的所有脚本
#[tauri::command]
pub fn list_scripts(storage: State<Storage>, server_id: i64) -> Vec<Script> {
    storage.get_scripts(server_id)
}

/// 获取单个脚本
#[tauri::command]
pub fn get_script(storage: State<Storage>, id: i64) -> Option<Script> {
    storage.get_script(id)
}

/// 获取启用的脚本（按类型）
#[tauri::command]
pub fn get_enabled_scripts(storage: State<Storage>, server_id: i64, script_type: String) -> Vec<Script> {
    storage.get_enabled_scripts(server_id, &script_type)
}

/// 创建脚本
#[tauri::command]
pub fn create_script(storage: State<Storage>, request: CreateScriptRequest) -> Result<i64, String> {
    storage.create_script(request)
}

/// 更新脚本
#[tauri::command]
pub fn update_script(storage: State<Storage>, request: UpdateScriptRequest) -> Result<(), String> {
    storage.update_script(request)
}

/// 删除脚本
#[tauri::command]
pub fn delete_script(storage: State<Storage>, id: i64) -> Result<(), String> {
    storage.delete_script(id)
}

/// 切换脚本启用状态
#[tauri::command]
pub fn toggle_script(storage: State<Storage>, id: i64, enabled: bool) -> Result<(), String> {
    storage.toggle_script(id, enabled)
}
