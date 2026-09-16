use parking_lot::RwLock;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, Transport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::db::models::MqttServer;
use crate::mqtt::frame::{encode_batch, PendingMessage};

/// 收发包大小上限（rumqttc 默认仅 10KB，超限会导致 poll 报错断连）
const MAX_PACKET_SIZE: usize = 10 * 1024 * 1024;
/// 单条消息推给前端的 payload 截断上限，超出部分不进入 IPC / 渲染
const MAX_EMIT_PAYLOAD: usize = 64 * 1024;
/// 系统根证书库缓存（Windows 上首次加载需读注册表解析上百张证书，50-200ms）
static SYSTEM_ROOT_STORE: OnceLock<rumqttc::tokio_rustls::rustls::RootCertStore> = OnceLock::new();

/// 向前端推送消息帧的 Channel（原始字节，见 `frame.rs`）
pub type MessageChannel = Channel<InvokeResponseBody>;
/// 所有连接共用的 Channel 槽位；前端注册前为 None，此时收到的消息直接丢弃
type ChannelSlot = Arc<RwLock<Option<MessageChannel>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub server_id: i64,
    pub status: String, // "disconnected", "connecting", "connected", "error"
    pub error: Option<String>,
}

struct ClientHandle {
    client: AsyncClient,
    shutdown_tx: mpsc::Sender<()>,
    /// 连接代数，用于防止旧 eventloop 退出时误删新连接的句柄、误发状态事件
    generation: u64,
    /// 真实连接状态：ConnAck 成功置 true，断开/出错/重连退避期间置 false。
    /// 断线期间发布/订阅据此立即失败，而不是被 rumqttc 静默入队。
    connected: Arc<AtomicBool>,
}

pub struct MqttManager {
    clients: Arc<RwLock<HashMap<i64, ClientHandle>>>,
    app_handle: AppHandle,
    next_generation: AtomicU64,
    message_channel: ChannelSlot,
}

impl MqttManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            next_generation: AtomicU64::new(0),
            message_channel: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置（或替换）向前端推送消息帧的 Channel
    ///
    /// 旧 Channel 被替换后随即 drop，Tauri 会向前端发送 end 标记，
    /// 前端侧对应的回调随之注销，不会残留。
    pub fn set_message_channel(&self, channel: MessageChannel) {
        *self.message_channel.write() = Some(channel);
    }

    pub async fn connect(&self, server: MqttServer) -> Result<(), String> {
        let server_id = server.id.ok_or("Server ID is required")?;

        // 如果已连接，先断开
        self.disconnect(server_id).await?;

        // 构建 MQTT 配置
        let client_id = server.client_id.unwrap_or_else(|| {
            format!("mqtt_client_{}", uuid::Uuid::new_v4())
        });

        // 防御性转换：正常路径上命令入口已校验，这里避免未来绕过校验时静默截断
        // （负 keep_alive 符号扩展后会让 ping 定时器与 CONNECT 包不一致，导致 broker 周期性踢线）
        let port = u16::try_from(server.port)
            .map_err(|_| format!("端口号必须在 1 到 65535 之间，当前为 {}", server.port))?;
        let keep_alive = u64::try_from(server.keep_alive)
            .map_err(|_| format!("Keep Alive 不能为负数，当前为 {}", server.keep_alive))?;

        let mut options = MqttOptions::new(client_id, &server.host, port);
        options.set_keep_alive(Duration::from_secs(keep_alive));
        options.set_clean_session(server.clean_session);
        options.set_max_packet_size(MAX_PACKET_SIZE, MAX_PACKET_SIZE);

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
                server.client_key_password.as_deref(),
            )?;
            options.set_transport(Transport::tls_with_config(tls_config));
        }

        // 创建客户端
        let (client, eventloop) = AsyncClient::new(options, 100);

        // 创建停止信号
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        // 连接代数：旧 eventloop 退出清理时校验，避免误删新连接的句柄
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let connected_flag = Arc::new(AtomicBool::new(false));

        // 保存客户端句柄
        {
            let mut clients = self.clients.write();
            clients.insert(
                server_id,
                ClientHandle {
                    client: client.clone(),
                    shutdown_tx,
                    generation,
                    connected: Arc::clone(&connected_flag),
                },
            );
        }

        // 发送连接中状态：必须在句柄写入之后，
        // 这样旧一代 eventloop 的 "disconnected" 要么先于本次 emit、要么被代数校验丢弃，
        // 不会出现 "connecting" 之后又冒出旧连接的 "disconnected"
        self.emit_state(server_id, "connecting", None);

        // 启动事件循环
        let app_handle = self.app_handle.clone();
        let clients = self.clients.clone();
        let message_channel = self.message_channel.clone();

        tokio::spawn(async move {
            Self::run_eventloop(
                server_id,
                generation,
                connected_flag,
                eventloop,
                shutdown_rx,
                app_handle,
                clients,
                message_channel,
            )
            .await;
        });

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_eventloop(
        server_id: i64,
        generation: u64,
        connected_flag: Arc<AtomicBool>,
        mut eventloop: EventLoop,
        mut shutdown_rx: mpsc::Receiver<()>,
        app_handle: AppHandle,
        clients: Arc<RwLock<HashMap<i64, ClientHandle>>>,
        message_channel: ChannelSlot,
    ) {
        // 攒批推送：累积 BATCH_MAX 条、累计 BATCH_MAX_BYTES 字节或到达 BATCH_INTERVAL 后
        // 编成一帧经 Channel 发给前端。按字节封顶是为了让单帧体积可控：
        // 50 条 64KB 的截断消息若只按条数攒批，一帧会超过 3MB
        const BATCH_MAX: usize = 50;
        const BATCH_MAX_BYTES: usize = 256 * 1024;
        const BATCH_INTERVAL: Duration = Duration::from_millis(30);
        // 瞬时错误的重连退避区间
        const INITIAL_RECONNECT_DELAY: Duration = Duration::from_millis(500);
        const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(10);
        // 从未连接成功时的最大重试次数（防止错误配置下无限重试）
        const MAX_INITIAL_ATTEMPTS: u32 = 5;

        let mut connected = false;
        let mut ever_connected = false;
        let mut initial_attempts: u32 = 0;
        let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
        let mut batch: Vec<PendingMessage> = Vec::new();
        let mut batch_bytes: usize = 0;
        // 攒批 flush 定时器：仅在批非空时参与 select，并在本批第一条消息到达时才 arm，
        // 消除空闲连接上每 30ms 一次的无谓唤醒（攒批语义仍是 50 条 / 30ms）
        let flush_timer = tokio::time::sleep(BATCH_INTERVAL);
        tokio::pin!(flush_timer);

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    Self::flush_batch(&message_channel, &mut batch, &mut batch_bytes);
                    connected_flag.store(false, Ordering::Release);
                    Self::emit_state_if_current(&app_handle, &clients, server_id, generation, "disconnected", None);
                    break;
                }
                _ = &mut flush_timer, if !batch.is_empty() => {
                    Self::flush_batch(&message_channel, &mut batch, &mut batch_bytes);
                }
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                            if ack.code == rumqttc::ConnectReturnCode::Success {
                                connected = true;
                                ever_connected = true;
                                reconnect_delay = INITIAL_RECONNECT_DELAY;
                                connected_flag.store(true, Ordering::Release);
                                Self::emit_state_if_current(&app_handle, &clients, server_id, generation, "connected", None);
                            } else {
                                connected_flag.store(false, Ordering::Release);
                                Self::emit_state_if_current(
                                    &app_handle,
                                    &clients,
                                    server_id,
                                    generation,
                                    "error",
                                    Some(format!("Connection refused: {:?}", ack.code)),
                                );
                                break;
                            }
                        }
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            let original_length = publish.payload.len();
                            let truncated = original_length > MAX_EMIT_PAYLOAD;
                            // Bytes 切片零拷贝，直到编帧时才真正复制一次
                            let payload = if truncated {
                                publish.payload.slice(..MAX_EMIT_PAYLOAD)
                            } else {
                                publish.payload
                            };
                            let message = PendingMessage {
                                server_id,
                                topic: publish.topic,
                                payload,
                                qos: publish.qos as u8,
                                retain: publish.retain,
                                timestamp: chrono::Utc::now().timestamp_millis(),
                                original_length,
                                truncated,
                            };
                            let batch_was_empty = batch.is_empty();
                            batch_bytes += message.encoded_size();
                            batch.push(message);
                            // 本批第一条消息到达时才重置定时器，保证等待上限为 BATCH_INTERVAL
                            if batch_was_empty {
                                flush_timer
                                    .as_mut()
                                    .reset(tokio::time::Instant::now() + BATCH_INTERVAL);
                            }
                            if batch.len() >= BATCH_MAX || batch_bytes >= BATCH_MAX_BYTES {
                                Self::flush_batch(&message_channel, &mut batch, &mut batch_bytes);
                            }
                        }
                        Ok(Event::Incoming(Packet::SubAck(_))) => {
                            // 订阅成功
                        }
                        Ok(Event::Incoming(Packet::PingResp)) => {
                            // Ping 响应
                        }
                        Err(e) => {
                            Self::flush_batch(&message_channel, &mut batch, &mut batch_bytes);
                            // 出错即视为断线：断线期间的发布/订阅立即失败，不再静默入队
                            connected_flag.store(false, Ordering::Release);

                            // 包超过大小上限属于持久性错误：broker 会重发同一条 retained 大包，
                            // 按瞬时错误退避重连会陷入"断连—重连—再断连"的无限循环
                            if Self::is_packet_size_error(&e) {
                                Self::emit_state_if_current(
                                    &app_handle,
                                    &clients,
                                    server_id,
                                    generation,
                                    "error",
                                    Some(format!(
                                        "消息超过大小上限（{} MB），已停止重连：{}",
                                        MAX_PACKET_SIZE / 1024 / 1024,
                                        e
                                    )),
                                );
                                break;
                            }

                            match e {
                                // 服务端明确拒绝（认证失败等）：致命错误，退出事件循环
                                rumqttc::ConnectionError::ConnectionRefused(code) => {
                                    Self::emit_state_if_current(
                                        &app_handle,
                                        &clients,
                                        server_id,
                                        generation,
                                        "error",
                                        Some(format!("Connection refused: {:?}", code)),
                                    );
                                    break;
                                }
                                // 客户端句柄已释放，无法继续
                                rumqttc::ConnectionError::RequestsDone => {
                                    Self::emit_state_if_current(&app_handle, &clients, server_id, generation, "disconnected", None);
                                    break;
                                }
                                // 瞬时错误（网络抖动等）：退避后继续轮询，
                                // rumqttc 的 poll 会自动重连，连上后前端按 connected 状态自动恢复订阅
                                e => {
                                    if !ever_connected {
                                        initial_attempts += 1;
                                        if initial_attempts >= MAX_INITIAL_ATTEMPTS {
                                            Self::emit_state_if_current(
                                                &app_handle,
                                                &clients,
                                                server_id,
                                                generation,
                                                "error",
                                                Some(format!("Failed to connect: {}", e)),
                                            );
                                            break;
                                        }
                                    }
                                    let detail = if connected {
                                        format!("Connection lost, reconnecting: {}", e)
                                    } else {
                                        format!("Reconnecting: {}", e)
                                    };
                                    connected = false;
                                    Self::emit_state_if_current(
                                        &app_handle,
                                        &clients,
                                        server_id,
                                        generation,
                                        "connecting",
                                        Some(detail),
                                    );
                                    // 退避等待，期间允许被断开操作打断
                                    tokio::select! {
                                        _ = shutdown_rx.recv() => {
                                            Self::emit_state_if_current(&app_handle, &clients, server_id, generation, "disconnected", None);
                                            break;
                                        }
                                        _ = tokio::time::sleep(reconnect_delay) => {}
                                    }
                                    reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        connected_flag.store(false, Ordering::Release);

        // 清理客户端：仅当句柄仍是本代连接时才移除，防止删掉新连接
        let mut clients = clients.write();
        if clients.get(&server_id).map(|h| h.generation) == Some(generation) {
            clients.remove(&server_id);
        }
    }

    /// 判断是否为"包超过大小上限"的持久性错误
    ///
    /// rumqttc 0.24 (v4) 在读到超限包时返回
    /// `ConnectionError::MqttState(StateError::Deserialization(Error::PayloadSizeLimitExceeded(_)))`；
    /// 等待 ConnAck 阶段的同类失败会被 framed 层包装成 `io::ErrorKind::InvalidData`。
    fn is_packet_size_error(e: &rumqttc::ConnectionError) -> bool {
        use rumqttc::mqttbytes::Error as MqttBytesError;
        use rumqttc::{ConnectionError, StateError};

        match e {
            ConnectionError::MqttState(StateError::Deserialization(
                MqttBytesError::PayloadSizeLimitExceeded(_),
            )) => true,
            ConnectionError::MqttState(StateError::OutgoingPacketTooLarge { .. }) => true,
            ConnectionError::Io(io_err) => {
                io_err.kind() == std::io::ErrorKind::InvalidData
                    && io_err
                        .to_string()
                        .to_lowercase()
                        .contains("payload size limit exceeded")
            }
            _ => false,
        }
    }

    /// 仅当自身仍是当前代连接时才 emit 状态事件
    ///
    /// 旧 eventloop 退出（shutdown / 出错）时无条件 emit "disconnected"，
    /// 可能晚于新一代连接的 "connecting"/"connected" 到达，让 UI 永久停在错误状态。
    fn emit_state_if_current(
        app_handle: &AppHandle,
        clients: &RwLock<HashMap<i64, ClientHandle>>,
        server_id: i64,
        generation: u64,
        status: &str,
        error: Option<String>,
    ) {
        let is_current = {
            let clients = clients.read();
            clients.get(&server_id).map(|h| h.generation) == Some(generation)
        };
        if !is_current {
            return;
        }
        Self::emit_state_static(app_handle, server_id, status, error);
    }

    /// 将攒批的消息编成一帧经 Channel 推给前端
    ///
    /// 帧走原始字节：Tauri 对超过 1KB 的原始负载改用 fetch 通道投递，
    /// 不再把数据拼进 JS 脚本求值，前端拿到的是 ArrayBuffer。
    /// `batch` 只清空不释放，容量在批次间复用。
    fn flush_batch(
        channel: &ChannelSlot,
        batch: &mut Vec<PendingMessage>,
        batch_bytes: &mut usize,
    ) {
        if batch.is_empty() {
            return;
        }
        let frame = encode_batch(batch);
        batch.clear();
        *batch_bytes = 0;

        // 先克隆出 Channel 再发送，不在持锁期间做 IPC
        let channel = channel.read().clone();
        if let Some(channel) = channel {
            let _ = channel.send(InvokeResponseBody::Raw(frame));
        }
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
        let client = self.connected_client(server_id)?;

        let qos = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err("QoS 必须为 0、1 或 2".to_string()),
        };

        client
            .publish(topic, qos, retain, payload)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn subscribe(&self, server_id: i64, topic: String, qos: u8) -> Result<(), String> {
        let client = self.connected_client(server_id)?;

        let qos = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err("QoS 必须为 0、1 或 2".to_string()),
        };

        client
            .subscribe(topic, qos)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn unsubscribe(&self, server_id: i64, topic: String) -> Result<(), String> {
        let client = self.connected_client(server_id)?;

        client.unsubscribe(topic).await.map_err(|e| e.to_string())
    }

    /// 取出处于"已连接"状态的客户端句柄
    ///
    /// 断线重连退避期间 rumqttc 仍会接受 publish/subscribe 并入队，
    /// 用户看到的是"发送成功"实则消息压在队列里，这里改为立即失败。
    fn connected_client(&self, server_id: i64) -> Result<AsyncClient, String> {
        let handle = {
            let clients = self.clients.read();
            clients
                .get(&server_id)
                .map(|h| (h.client.clone(), h.connected.load(Ordering::Acquire)))
        };

        match handle {
            Some((client, true)) => Ok(client),
            Some((_, false)) => Err("未连接：正在重连中，请稍后重试".to_string()),
            None => Err("未连接到服务器".to_string()),
        }
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

    /// 是否真正处于已连接状态（重连退避期间为 false，调用方据此跳过订阅动作）
    pub fn is_connected(&self, server_id: i64) -> bool {
        let clients = self.clients.read();
        clients
            .get(&server_id)
            .map(|h| h.connected.load(Ordering::Acquire))
            .unwrap_or(false)
    }

    /// 构建 TLS 配置
    fn build_tls_config(
        ca_cert: Option<&str>,
        client_cert: Option<&str>,
        client_key: Option<&str>,
        client_key_password: Option<&str>,
    ) -> Result<rumqttc::TlsConfiguration, String> {
        use std::io::BufReader;

        // 系统根证书只在首次连接时加载一次，之后直接克隆缓存
        let mut root_cert_store = SYSTEM_ROOT_STORE
            .get_or_init(|| {
                let mut store = rumqttc::tokio_rustls::rustls::RootCertStore::empty();
                let native_certs = rustls_native_certs::load_native_certs();
                for cert in native_certs.certs {
                    let _ = store.add(cert);
                }
                store
            })
            .clone();

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
                let key = Self::parse_private_key(key_pem, client_key_password)?;

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

    /// 解析私钥，支持加密和未加密格式
    fn parse_private_key(
        key_pem: &str,
        password: Option<&str>,
    ) -> Result<rumqttc::tokio_rustls::rustls::pki_types::PrivateKeyDer<'static>, String> {
        use std::io::BufReader;
        use rumqttc::tokio_rustls::rustls::pki_types::PrivateKeyDer;
        use pkcs8::der::Decode;

        // 首先尝试解析未加密的私钥
        let mut key_reader = BufReader::new(key_pem.as_bytes());
        if let Ok(Some(key)) = rustls_pemfile::private_key(&mut key_reader) {
            return Ok(key);
        }

        // 如果提供了密码，尝试解析加密的私钥
        if let Some(pwd) = password {
            if !pwd.is_empty() {
                // 尝试从 PEM 解析加密的私钥
                let pem = pem::parse(key_pem)
                    .map_err(|e| format!("Failed to parse PEM: {}", e))?;
                
                if pem.tag() == "ENCRYPTED PRIVATE KEY" {
                    // 解析加密的 PKCS#8
                    let encrypted = pkcs8::EncryptedPrivateKeyInfo::from_der(pem.contents())
                        .map_err(|e| format!("Failed to parse encrypted private key: {}", e))?;
                    
                    let decrypted = encrypted.decrypt(pwd)
                        .map_err(|e| format!("Failed to decrypt private key (wrong password?): {}", e))?;
                    
                    let der_bytes = decrypted.as_bytes().to_vec();
                    
                    return Ok(PrivateKeyDer::Pkcs8(der_bytes.into()));
                }
            }
        }

        Err("No valid private key found in PEM. If the key is encrypted, please provide the password.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumqttc::mqttbytes::Error as MqttBytesError;
    use rumqttc::{ConnectionError, StateError};

    /// 包超限必须被识别为持久性错误，否则 broker 重发 retained 大包会导致无限重连
    #[test]
    fn packet_size_errors_are_classified_as_persistent() {
        assert!(MqttManager::is_packet_size_error(&ConnectionError::MqttState(
            StateError::Deserialization(MqttBytesError::PayloadSizeLimitExceeded(20 * 1024 * 1024))
        )));
        assert!(MqttManager::is_packet_size_error(&ConnectionError::MqttState(
            StateError::OutgoingPacketTooLarge {
                pkt_size: 20 * 1024 * 1024,
                max: MAX_PACKET_SIZE,
            }
        )));
        // 等待 ConnAck 阶段由 framed 层包装成的 InvalidData
        assert!(MqttManager::is_packet_size_error(&ConnectionError::Io(
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                MqttBytesError::PayloadSizeLimitExceeded(20 * 1024 * 1024).to_string(),
            )
        )));
    }

    /// 瞬时错误必须继续走指数退避重连，不能被误判成持久性错误
    #[test]
    fn transient_errors_are_not_classified_as_packet_size_errors() {
        assert!(!MqttManager::is_packet_size_error(&ConnectionError::NetworkTimeout));
        assert!(!MqttManager::is_packet_size_error(&ConnectionError::FlushTimeout));
        assert!(!MqttManager::is_packet_size_error(&ConnectionError::RequestsDone));
        assert!(!MqttManager::is_packet_size_error(&ConnectionError::Io(
            std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset by peer")
        )));
        assert!(!MqttManager::is_packet_size_error(&ConnectionError::MqttState(
            StateError::AwaitPingResp
        )));
    }
}
