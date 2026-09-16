use crate::db::Storage;
use crate::mqtt::{MessageChannel, MqttManager};
use tauri::State;

/// 注册接收消息的 Channel
///
/// 前端启动时创建一个 Channel 交给后端，之后所有 Server 收到的消息都攒批编成
/// 二进制帧经这一条 Channel 推送（比事件机制少一次 JSON 序列化与脚本求值，
/// 且大帧走 fetch 通道而非 eval）。重复注册时替换旧 Channel。
#[tauri::command]
pub fn register_message_channel(
    mqtt: State<'_, MqttManager>,
    channel: MessageChannel,
) -> Result<(), String> {
    mqtt.set_message_channel(channel);
    Ok(())
}

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
