use parking_lot::RwLock;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::db::models::MqttServer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub server_id: i64,
    pub status: String, // "disconnected", "connecting", "connected", "error"
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedMessage {
    pub server_id: i64,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retain: bool,
    pub timestamp: String,
}

struct ClientHandle {
    client: AsyncClient,
    shutdown_tx: mpsc::Sender<()>,
}

pub struct MqttManager {
    clients: Arc<RwLock<HashMap<i64, ClientHandle>>>,
    app_handle: AppHandle,
}

impl MqttManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
        }
    }

    pub async fn connect(&self, server: MqttServer) -> Result<(), String> {
        let server_id = server.id.ok_or("Server ID is required")?;

        // 如果已连接，先断开
        self.disconnect(server_id).await?;

        // 发送连接中状态
        self.emit_state(server_id, "connecting", None);

        // 构建 MQTT 配置
        let client_id = server.client_id.unwrap_or_else(|| {
            format!("mqtt_client_{}", uuid::Uuid::new_v4())
        });

        let mut options = MqttOptions::new(client_id, &server.host, server.port as u16);
        options.set_keep_alive(Duration::from_secs(server.keep_alive as u64));
        options.set_clean_session(server.clean_session);

        if let (Some(username), Some(password)) = (server.username.as_ref(), server.password.as_ref())
        {
            if !username.is_empty() {
                options.set_credentials(username, password);
            }
        }

        // 创建客户端
        let (client, eventloop) = AsyncClient::new(options, 100);

        // 创建停止信号
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        // 保存客户端句柄
        {
            let mut clients = self.clients.write();
            clients.insert(
                server_id,
                ClientHandle {
                    client: client.clone(),
                    shutdown_tx,
                },
            );
        }

        // 启动事件循环
        let app_handle = self.app_handle.clone();
        let clients = self.clients.clone();

        tokio::spawn(async move {
            Self::run_eventloop(server_id, eventloop, shutdown_rx, app_handle, clients).await;
        });

        Ok(())
    }

    async fn run_eventloop(
        server_id: i64,
        mut eventloop: EventLoop,
        mut shutdown_rx: mpsc::Receiver<()>,
        app_handle: AppHandle,
        clients: Arc<RwLock<HashMap<i64, ClientHandle>>>,
    ) {
        let mut connected = false;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    Self::emit_state_static(&app_handle, server_id, "disconnected", None);
                    break;
                }
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                            if ack.code == rumqttc::ConnectReturnCode::Success {
                                connected = true;
                                Self::emit_state_static(&app_handle, server_id, "connected", None);
                            } else {
                                Self::emit_state_static(
                                    &app_handle,
                                    server_id,
                                    "error",
                                    Some(format!("Connection refused: {:?}", ack.code)),
                                );
                                break;
                            }
                        }
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            let msg = ReceivedMessage {
                                server_id,
                                topic: publish.topic.clone(),
                                payload: publish.payload.to_vec(),
                                qos: publish.qos as u8,
                                retain: publish.retain,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };
                            let _ = app_handle.emit("mqtt-message", msg);
                        }
                        Ok(Event::Incoming(Packet::SubAck(_))) => {
                            // 订阅成功
                        }
                        Ok(Event::Incoming(Packet::PingResp)) => {
                            // Ping 响应
                        }
                        Err(e) => {
                            if connected {
                                Self::emit_state_static(
                                    &app_handle,
                                    server_id,
                                    "error",
                                    Some(format!("Connection error: {}", e)),
                                );
                            } else {
                                Self::emit_state_static(
                                    &app_handle,
                                    server_id,
                                    "error",
                                    Some(format!("Failed to connect: {}", e)),
                                );
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        // 清理客户端
        let mut clients = clients.write();
        clients.remove(&server_id);
    }

    pub async fn disconnect(&self, server_id: i64) -> Result<(), String> {
        let handle = {
            let clients = self.clients.read();
            clients.get(&server_id).map(|h| h.shutdown_tx.clone())
        };

        if let Some(tx) = handle {
            let _ = tx.send(()).await;
        }

        Ok(())
    }

    pub async fn publish(
        &self,
        server_id: i64,
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
    ) -> Result<(), String> {
        let client = {
            let clients = self.clients.read();
            clients.get(&server_id).map(|h| h.client.clone())
        };

        let client = client.ok_or("Not connected")?;

        let qos = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err("Invalid QoS".to_string()),
        };

        client
            .publish(topic, qos, retain, payload)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn subscribe(&self, server_id: i64, topic: String, qos: u8) -> Result<(), String> {
        let client = {
            let clients = self.clients.read();
            clients.get(&server_id).map(|h| h.client.clone())
        };

        let client = client.ok_or("Not connected")?;

        let qos = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err("Invalid QoS".to_string()),
        };

        client
            .subscribe(topic, qos)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn unsubscribe(&self, server_id: i64, topic: String) -> Result<(), String> {
        let client = {
            let clients = self.clients.read();
            clients.get(&server_id).map(|h| h.client.clone())
        };

        let client = client.ok_or("Not connected")?;

        client.unsubscribe(topic).await.map_err(|e| e.to_string())
    }

    fn emit_state(&self, server_id: i64, status: &str, error: Option<String>) {
        Self::emit_state_static(&self.app_handle, server_id, status, error);
    }

    fn emit_state_static(
        app_handle: &AppHandle,
        server_id: i64,
        status: &str,
        error: Option<String>,
    ) {
        let state = ConnectionState {
            server_id,
            status: status.to_string(),
            error,
        };
        let _ = app_handle.emit("mqtt-connection-state", state);
    }

    pub fn is_connected(&self, server_id: i64) -> bool {
        let clients = self.clients.read();
        clients.contains_key(&server_id)
    }
}
