use crate::db::models::{NewSubscription, Subscription};
use crate::db::Database;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn add_subscription(
    db: State<'_, Database>,
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    topic: String,
    qos: i32,
) -> Result<Subscription, String> {
    // 先添加到数据库
    let subscription = {
        let new_sub = NewSubscription {
            server_id,
            topic: topic.clone(),
            qos,
            is_active: true,
        };
        db.insert_subscription(&new_sub)
            .map_err(|e| e.to_string())?
    };

    // 如果已连接，则订阅主题
    if mqtt_manager.is_connected(server_id) {
        mqtt_manager
            .subscribe(server_id, topic, qos as u8)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(subscription)
}

#[tauri::command]
pub async fn remove_subscription(
    db: State<'_, Database>,
    mqtt_manager: State<'_, MqttManager>,
    subscription_id: i64,
    server_id: i64,
    topic: String,
) -> Result<(), String> {
    // 如果已连接，则取消订阅
    if mqtt_manager.is_connected(server_id) {
        mqtt_manager
            .unsubscribe(server_id, topic)
            .await
            .map_err(|e| e.to_string())?;
    }

    // 从数据库删除
    db.delete_subscription(subscription_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_subscriptions(
    db: State<'_, Database>,
    server_id: i64,
) -> Result<Vec<Subscription>, String> {
    db.get_subscriptions_by_server(server_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_subscription(
    db: State<'_, Database>,
    mqtt_manager: State<'_, MqttManager>,
    subscription_id: i64,
    server_id: i64,
    topic: String,
    qos: i32,
    is_active: bool,
) -> Result<(), String> {
    // 更新数据库状态
    db.update_subscription_status(subscription_id, is_active)
        .map_err(|e| e.to_string())?;

    // 执行订阅/取消订阅操作
    if mqtt_manager.is_connected(server_id) {
        if is_active {
            mqtt_manager
                .subscribe(server_id, topic, qos as u8)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            mqtt_manager
                .unsubscribe(server_id, topic)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
