use tauri::State;
use crate::db::models::{EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest};
use crate::db::Storage;

/// 获取服务器的所有环境变量
#[tauri::command]
pub fn list_env_variables(storage: State<Storage>, server_id: i64) -> Vec<EnvVariable> {
    storage.get_env_variables(server_id)
}

/// 获取单个环境变量
#[tauri::command]
pub fn get_env_variable(storage: State<Storage>, id: i64) -> Option<EnvVariable> {
    storage.get_env_variable(id)
}

/// 创建环境变量
#[tauri::command]
pub fn create_env_variable(storage: State<Storage>, request: CreateEnvVariableRequest) -> Result<i64, String> {
    storage.create_env_variable(request)
}

/// 更新环境变量
#[tauri::command]
pub fn update_env_variable(storage: State<Storage>, request: UpdateEnvVariableRequest) -> Result<(), String> {
    storage.update_env_variable(request)
}

/// 删除环境变量
#[tauri::command]
pub fn delete_env_variable(storage: State<Storage>, id: i64) -> Result<(), String> {
    storage.delete_env_variable(id)
}
