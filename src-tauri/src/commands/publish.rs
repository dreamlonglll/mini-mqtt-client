use crate::db::models::{MessageHistory, NewMessageHistory, PublishPayload};
use crate::db::Database;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn publish_message(
    db: State<'_, Database>,
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    message: PublishPayload,
) -> Result<MessageHistory, String> {
    // 转换消息内容
    let payload_bytes = match message.format.as_str() {
        "hex" => hex::decode(message.payload.replace(" ", ""))
            .map_err(|e| format!("HEX解码失败: {}", e))?,
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
    let history = {
        let new_history = NewMessageHistory {
            server_id,
            topic: message.topic,
            payload: message.payload,
            payload_format: message.format,
            direction: "publish".to_string(),
            qos: message.qos,
            retain: message.retain,
        };
        db.insert_message_history(&new_history)
            .map_err(|e| e.to_string())?
    };

    Ok(history)
}

#[tauri::command]
pub async fn get_message_history(
    db: State<'_, Database>,
    server_id: i64,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<MessageHistory>, String> {
    db.get_message_history(server_id, limit.unwrap_or(100), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_message_history(db: State<'_, Database>, server_id: i64) -> Result<(), String> {
    db.clear_message_history(server_id)
        .map_err(|e| e.to_string())
}
