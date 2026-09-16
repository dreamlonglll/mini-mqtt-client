use crate::db::models::PublishPayload;
use crate::mqtt::MqttManager;
use tauri::State;

/// 发布一条消息
///
/// 发布历史只保留在前端消息列表里，后端不再重复留存一份无人读取的副本。
#[tauri::command]
pub async fn publish_message(
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    message: PublishPayload,
) -> Result<(), String> {
    // 转换消息内容（HEX 边过滤空白边收集，避免 replace 产生的中间字符串）
    let payload_bytes = match message.format.as_str() {
        "hex" => {
            let cleaned: Vec<u8> = message
                .payload
                .bytes()
                .filter(|b| !b.is_ascii_whitespace())
                .collect();
            hex::decode(cleaned).map_err(|e| format!("HEX decode failed: {}", e))?
        }
        _ => message.payload.into_bytes(),
    };

    mqtt_manager
        .publish(
            server_id,
            message.topic,
            payload_bytes,
            message.qos as u8,
            message.retain,
        )
        .await
        .map_err(|e| e.to_string())
}
