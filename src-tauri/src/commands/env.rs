use tauri::State;
use crate::db::models::{EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest};
use crate::db::Storage;

/// 获取服务器的所有环境变量
#[tauri::command]
pub async fn list_env_variables(storage: State<'_, Storage>, server_id: i64) -> Result<serde_json::Value, String> {
    storage.get_env_variables_json(server_id)
}

/// 获取单个环境变量
#[tauri::command]
pub async fn get_env_variable(storage: State<'_, Storage>, id: i64) -> Result<Option<EnvVariable>, String> {
    Ok(storage.get_env_variable(id))
}

/// 创建环境变量
#[tauri::command]
pub async fn create_env_variable(storage: State<'_, Storage>, request: CreateEnvVariableRequest) -> Result<i64, String> {
    storage.create_env_variable(request)
}

/// 更新环境变量
#[tauri::command]
pub async fn update_env_variable(storage: State<'_, Storage>, request: UpdateEnvVariableRequest) -> Result<(), String> {
    storage.update_env_variable(request)
}

/// 删除环境变量
#[tauri::command]
pub async fn delete_env_variable(storage: State<'_, Storage>, id: i64) -> Result<(), String> {
    storage.delete_env_variable(id)
}
