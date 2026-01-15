use crate::db::models::MqttServer;
use crate::db::Database;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn mqtt_connect(
    db: State<'_, Database>,
    mqtt: State<'_, MqttManager>,
    server_id: i64,
) -> Result<(), String> {
    // 从数据库获取 server 配置
    let server = {
        let conn = db.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, host, port, protocol_version, username, password, 
                 client_id, keep_alive, clean_session, use_tls, ca_cert, client_cert, 
                 client_key, created_at, updated_at FROM mqtt_servers WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;

        stmt.query_row([server_id], |row| {
            Ok(MqttServer {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                protocol_version: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                client_id: row.get(7)?,
                keep_alive: row.get(8)?,
                clean_session: row.get::<_, i32>(9)? == 1,
                use_tls: row.get::<_, i32>(10)? == 1,
                ca_cert: row.get(11)?,
                client_cert: row.get(12)?,
                client_key: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?
    };

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
