use crate::commands::validation::validate_server;
use crate::db::models::MqttServer;
use crate::db::Storage;
use tauri::State;

#[tauri::command]
pub async fn get_servers(storage: State<'_, Storage>) -> Result<serde_json::Value, String> {
    storage.get_servers_json()
}

#[tauri::command]
pub async fn create_server(storage: State<'_, Storage>, server: MqttServer) -> Result<i64, String> {
    // 先校验后持久化，非法配置不落库
    validate_server(&server)?;
    storage.create_server(server)
}

#[tauri::command]
pub async fn update_server(storage: State<'_, Storage>, server: MqttServer) -> Result<(), String> {
    // 先校验后持久化，非法配置不落库
    validate_server(&server)?;
    storage.update_server(server)
}

#[tauri::command]
pub async fn delete_server(storage: State<'_, Storage>, id: i64) -> Result<(), String> {
    storage.delete_server(id)
}

#[tauri::command]
pub async fn get_server(storage: State<'_, Storage>, id: i64) -> Result<Option<MqttServer>, String> {
    Ok(storage.get_server(id))
}
