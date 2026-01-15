use crate::db::Storage;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn mqtt_connect(
    storage: State<'_, Storage>,
    mqtt: State<'_, MqttManager>,
    server_id: i64,
) -> Result<(), String> {
    // 从存储获取 server 配置
    let server = storage
        .get_server(server_id)
        .ok_or("Server not found")?;

    mqtt.connect(server).await
}

#[tauri::command]
pub async fn mqtt_disconnect(mqtt: State<'_, MqttManager>, server_id: i64) -> Result<(), String> {
    mqtt.disconnect(server_id).await
}

#[tauri::command]
pub async fn mqtt_publish(
    mqtt: State<'_, MqttManager>,
    server_id: i64,
    topic: String,
    payload: Vec<u8>,
    qos: u8,
    retain: bool,
) -> Result<(), String> {
    mqtt.publish(server_id, topic, payload, qos, retain).await
}

#[tauri::command]
pub async fn mqtt_subscribe(
    mqtt: State<'_, MqttManager>,
    server_id: i64,
    topic: String,
    qos: u8,
) -> Result<(), String> {
    mqtt.subscribe(server_id, topic, qos).await
}

#[tauri::command]
pub async fn mqtt_unsubscribe(
    mqtt: State<'_, MqttManager>,
    server_id: i64,
    topic: String,
) -> Result<(), String> {
    mqtt.unsubscribe(server_id, topic).await
}

#[tauri::command]
pub fn mqtt_is_connected(mqtt: State<'_, MqttManager>, server_id: i64) -> bool {
    mqtt.is_connected(server_id)
}
