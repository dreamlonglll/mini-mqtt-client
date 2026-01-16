use crate::db::models::{MessageHistory, PublishPayload};
use crate::db::Storage;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn publish_message(
    storage: State<'_, Storage>,
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    message: PublishPayload,
) -> Result<MessageHistory, String> {
    // 转换消息内容
    let payload_bytes = match message.format.as_str() {
        "hex" => hex::decode(message.payload.replace(" ", ""))
            .map_err(|e| format!("HEX decode failed: {}", e))?,
        _ => message.payload.as_bytes().to_vec(),
    };

    // 发布消息
    mqtt_manager
        .publish(
            server_id,
            message.topic.clone(),
            payload_bytes,
            message.qos as u8,
            message.retain,
        )
        .await
        .map_err(|e| e.to_string())?;

    // 保存到历史记录
    let history = MessageHistory {
        id: None,
        server_id,
        topic: message.topic,
        payload: Some(message.payload),
        payload_format: Some(message.format),
        direction: "publish".to_string(),
        qos: message.qos,
        retain: message.retain,
        created_at: None,
    };

    storage.create_message(history)
}

#[tauri::command]
pub async fn get_message_history(
    storage: State<'_, Storage>,
    server_id: i64,
    limit: Option<usize>,
) -> Result<Vec<MessageHistory>, String> {
    Ok(storage.get_messages(server_id, limit.unwrap_or(100)))
}

#[tauri::command]
pub async fn clear_message_history(
    storage: State<'_, Storage>,
    server_id: i64,
) -> Result<(), String> {
    storage.clear_messages(server_id)
}
