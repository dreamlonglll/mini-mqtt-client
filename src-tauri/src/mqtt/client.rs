use parking_lot::RwLock;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, Transport};
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

        // 配置 TLS
        if server.use_tls {
            let tls_config = Self::build_tls_config(
                server.ca_cert.as_deref(),
                server.client_cert.as_deref(),
                server.client_key.as_deref(),
            )?;
            options.set_transport(Transport::tls_with_config(tls_config));
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

    /// 构建 TLS 配置
    fn build_tls_config(
        ca_cert: Option<&str>,
        client_cert: Option<&str>,
        client_key: Option<&str>,
    ) -> Result<rumqttc::TlsConfiguration, String> {
        use std::io::BufReader;

        // 创建根证书存储
        let mut root_cert_store = rumqttc::tokio_rustls::rustls::RootCertStore::empty();

        // 添加系统默认根证书
        let native_certs = rustls_native_certs::load_native_certs();
        for cert in native_certs.certs {
            let _ = root_cert_store.add(cert);
        }

        // 如果提供了自定义 CA 证书，添加到根存储
        if let Some(ca_pem) = ca_cert {
            if !ca_pem.trim().is_empty() {
                let mut reader = BufReader::new(ca_pem.as_bytes());
                for cert in rustls_pemfile::certs(&mut reader) {
                    let cert = cert.map_err(|e| format!("Failed to parse CA certificate: {}", e))?;
                    root_cert_store.add(cert).map_err(|e| format!("Failed to add CA certificate: {}", e))?;
                }
            }
        }

        // 构建客户端配置
        let builder = rumqttc::tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store);

        let client_config = match (client_cert, client_key) {
            (Some(cert_pem), Some(key_pem)) if !cert_pem.trim().is_empty() && !key_pem.trim().is_empty() => {
                // 解析客户端证书
                let mut cert_reader = BufReader::new(cert_pem.as_bytes());
                let mut certs = Vec::new();
                for cert in rustls_pemfile::certs(&mut cert_reader) {
                    let cert = cert.map_err(|e| format!("Failed to parse client certificate: {}", e))?;
                    certs.push(cert);
                }

                // 解析客户端私钥
                let mut key_reader = BufReader::new(key_pem.as_bytes());
                let key = rustls_pemfile::private_key(&mut key_reader)
                    .map_err(|e| format!("Failed to read private key: {}", e))?
                    .ok_or("No private key found in PEM")?;

                builder
                    .with_client_auth_cert(certs, key)
                    .map_err(|e| format!("Failed to configure client auth: {}", e))?
            }
            _ => {
                // 无客户端认证
                builder.with_no_client_auth()
            }
        };

        Ok(rumqttc::TlsConfiguration::Rustls(Arc::new(client_config)))
    }
}
