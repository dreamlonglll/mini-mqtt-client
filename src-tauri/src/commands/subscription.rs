use crate::db::models::{Subscription, UpdateSubscriptionRequest};
use crate::db::Storage;
use crate::mqtt::MqttManager;
use tauri::State;

#[tauri::command]
pub async fn add_subscription(
    storage: State<'_, Storage>,
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    topic: String,
    qos: i32,
) -> Result<Subscription, String> {
    // 创建订阅
    let sub = Subscription {
        id: None,
        server_id,
        topic: topic.clone(),
        qos,
        is_active: true,
        color: None,
        created_at: None,
    };

    let subscription = storage.create_subscription(sub)?;

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
    storage: State<'_, Storage>,
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

    // 从存储删除
    storage.delete_subscription(subscription_id)
}

#[tauri::command]
pub async fn get_subscriptions(
    storage: State<'_, Storage>,
    server_id: i64,
) -> Result<Vec<Subscription>, String> {
    Ok(storage.get_subscriptions(server_id))
}

#[tauri::command]
pub async fn toggle_subscription(
    storage: State<'_, Storage>,
    mqtt_manager: State<'_, MqttManager>,
    subscription_id: i64,
    server_id: i64,
    topic: String,
    qos: i32,
    is_active: bool,
) -> Result<(), String> {
    // 更新存储状态
    storage.update_subscription_status(subscription_id, is_active)?;

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

#[tauri::command]
pub async fn update_subscription(
    storage: State<'_, Storage>,
    mqtt_manager: State<'_, MqttManager>,
    server_id: i64,
    old_topic: String,
    request: UpdateSubscriptionRequest,
) -> Result<Subscription, String> {
    let new_topic = request.topic.clone();
    let new_qos = request.qos;

    // 更新存储
    let subscription = storage.update_subscription(request)?;

    // 如果已连接且 topic 或 qos 改变，需要重新订阅
    if mqtt_manager.is_connected(server_id) && subscription.is_active {
        // 如果 topic 改变了，先取消订阅旧的
        if let Some(ref topic) = new_topic {
            if topic != &old_topic {
                mqtt_manager
                    .unsubscribe(server_id, old_topic)
                    .await
                    .map_err(|e| e.to_string())?;
                mqtt_manager
                    .subscribe(server_id, topic.clone(), subscription.qos as u8)
                    .await
                    .map_err(|e| e.to_string())?;
            } else if new_qos.is_some() {
                // topic 没变但 qos 变了，重新订阅
                mqtt_manager
                    .subscribe(server_id, topic.clone(), subscription.qos as u8)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        } else if new_qos.is_some() {
            // 只有 qos 变了
            mqtt_manager
                .subscribe(server_id, subscription.topic.clone(), subscription.qos as u8)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(subscription)
}
