//! MQTT connection manager.
//!
//! Every connection runs its event loop on a shared tokio runtime and reports
//! back through an unbounded channel of [`MqttEvent`]s, which the UI drains on
//! its own executor. Publishing/subscribing uses the non-blocking `try_*`
//! client methods, so they can be called directly from the UI thread.
//!
//! Supports MQTT 3.1.1 and 5.0 over TCP, TLS, WebSocket and secure WebSocket.

use super::models::{MqttServer, Transport};
use chrono::{DateTime, Local};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui_kit::SharedString;
use parking_lot::Mutex;
use rumqttc::{TlsConfiguration, Transport as RumqttTransport};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_rustls::rustls;

const REQUEST_CHANNEL_CAPACITY: usize = 256;
const CONNECTION_TIMEOUT_SECS: u64 = 10;
/// Received messages not yet taken by the UI. Beyond these limits the OLDEST
/// queued messages are dropped (and counted) instead of growing memory without
/// bound, so after an overload the UI still converges to the newest data.
const MAX_PENDING_MESSAGES: usize = 50_000;
const MAX_PENDING_BYTES: usize = 256 * 1024 * 1024;
/// rumqttc's v5 client preallocates its inflight table from this limit
/// (u16::MAX when unset, ~13 MB per connection).
const MAX_OUTGOING_INFLIGHT: u16 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl ConnectionStatus {
    pub fn label(&self) -> SharedString {
        match self {
            ConnectionStatus::Disconnected => tr!("status.disconnected"),
            ConnectionStatus::Connecting => tr!("status.connecting"),
            ConnectionStatus::Connected => tr!("status.connected"),
            ConnectionStatus::Error(_) => tr!("status.error"),
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected)
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, ConnectionStatus::Connecting | ConnectionStatus::Connected)
    }
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub server_id: i64,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retain: bool,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone)]
pub enum MqttEvent {
    Status {
        server_id: i64,
        status: ConnectionStatus,
    },
    /// Received messages are waiting; fetch them with [`MqttManager::take_messages`].
    /// Sent when the message queue goes from empty to non-empty.
    MessagesReady,
    /// Non-fatal problem worth telling the user about (e.g. rejected subscription).
    Warning {
        server_id: i64,
        message: String,
    },
}

enum Client {
    V4(rumqttc::AsyncClient),
    V5(rumqttc::v5::AsyncClient),
}

fn qos_v4(qos: u8) -> Result<rumqttc::QoS, String> {
    rumqttc::qos(qos).map_err(|_| tr!("mqtt.invalid_qos", qos = qos).to_string())
}

fn qos_v5(qos: u8) -> Result<rumqttc::v5::mqttbytes::QoS, String> {
    rumqttc::v5::mqttbytes::qos(qos).ok_or_else(|| tr!("mqtt.invalid_qos", qos = qos).to_string())
}

impl Client {
    fn publish(&self, topic: String, qos: u8, retain: bool, payload: Vec<u8>) -> Result<(), String> {
        match self {
            Client::V4(c) => c
                .try_publish(topic, qos_v4(qos)?, retain, payload)
                .map_err(|e| e.to_string()),
            Client::V5(c) => c
                .try_publish(topic, qos_v5(qos)?, retain, payload)
                .map_err(|e| e.to_string()),
        }
    }

    fn subscribe(&self, topic: String, qos: u8) -> Result<(), String> {
        match self {
            Client::V4(c) => c.try_subscribe(topic, qos_v4(qos)?).map_err(|e| e.to_string()),
            Client::V5(c) => c.try_subscribe(topic, qos_v5(qos)?).map_err(|e| e.to_string()),
        }
    }

    fn unsubscribe(&self, topic: String) -> Result<(), String> {
        match self {
            Client::V4(c) => c.try_unsubscribe(topic).map_err(|e| e.to_string()),
            Client::V5(c) => c.try_unsubscribe(topic).map_err(|e| e.to_string()),
        }
    }
}

struct Handle {
    client: Client,
    generation: u64,
    connected: Arc<AtomicBool>,
    shutdown: Option<oneshot::Sender<()>>,
}

struct Inner {
    clients: Mutex<HashMap<i64, Handle>>,
    events: UnboundedSender<MqttEvent>,
    next_generation: AtomicU64,
    queue: Mutex<MessageQueue>,
    dropped: AtomicU64,
}

#[derive(Default)]
struct MessageQueue {
    messages: VecDeque<IncomingMessage>,
    bytes: usize,
}

impl Inner {
    fn is_current(&self, server_id: i64, generation: u64) -> bool {
        self.clients
            .lock()
            .get(&server_id)
            .is_some_and(|h| h.generation == generation)
    }

    /// Emit an event only if it comes from the live connection of the server,
    /// so a loop that is being torn down cannot overwrite newer state.
    fn emit(&self, server_id: i64, generation: u64, event: MqttEvent) {
        if self.is_current(server_id, generation) {
            let _ = self.events.unbounded_send(event);
        }
    }

    /// Queue a received message for the UI, evicting the oldest queued
    /// messages when the backlog is full.
    fn emit_message(&self, server_id: i64, generation: u64, message: IncomingMessage) {
        if !self.is_current(server_id, generation) {
            return;
        }
        let was_empty = {
            let mut queue = self.queue.lock();
            let was_empty = queue.messages.is_empty();
            queue.bytes += message.payload.len();
            queue.messages.push_back(message);
            while queue.messages.len() > 1
                && (queue.messages.len() > MAX_PENDING_MESSAGES || queue.bytes > MAX_PENDING_BYTES)
            {
                if let Some(old) = queue.messages.pop_front() {
                    queue.bytes -= old.payload.len();
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            was_empty
        };
        if was_empty {
            let _ = self.events.unbounded_send(MqttEvent::MessagesReady);
        }
    }

    fn emit_status(&self, server_id: i64, generation: u64, status: ConnectionStatus) {
        self.emit(server_id, generation, MqttEvent::Status { server_id, status });
    }

    fn remove_if_current(&self, server_id: i64, generation: u64) {
        let mut clients = self.clients.lock();
        if clients.get(&server_id).is_some_and(|h| h.generation == generation) {
            clients.remove(&server_id);
        }
    }
}

pub struct MqttManager {
    runtime: tokio::runtime::Runtime,
    inner: Arc<Inner>,
}

impl MqttManager {
    /// Create the manager and the receiving end of its event stream.
    pub fn new() -> (Self, UnboundedReceiver<MqttEvent>) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("mqtt-io")
            .enable_all()
            .build()
            .expect("failed to start tokio runtime");
        let (tx, rx) = unbounded();
        (
            Self {
                runtime,
                inner: Arc::new(Inner {
                    clients: Mutex::new(HashMap::new()),
                    events: tx,
                    next_generation: AtomicU64::new(1),
                    queue: Mutex::new(MessageQueue::default()),
                    dropped: AtomicU64::new(0),
                }),
            },
            rx,
        )
    }

    pub fn connect(&self, server: &MqttServer) -> Result<(), String> {
        let server_id = server.id.ok_or("Server ID is required")?;
        self.shutdown(server_id);

        let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let client_id = server
            .client_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("mqtt_client_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]));

        let transport = build_transport(server)?;
        let host = broker_host(server);
        let port = u16::try_from(server.port).map_err(|_| tr!("mqtt.invalid_port").to_string())?;
        let keep_alive = keep_alive_for(server);
        let credentials = server
            .username
            .as_deref()
            .filter(|u| !u.is_empty())
            .map(|u| (u.to_string(), server.password.clone().unwrap_or_default()));

        let connected = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let inner = self.inner.clone();
        // Enter the runtime: rumqttc's constructors and TLS setup expect it.
        let _guard = self.runtime.enter();

        let client = if server.is_v5() {
            let mut options = rumqttc::v5::MqttOptions::new(client_id, host, port);
            options
                .set_keep_alive(keep_alive)
                .set_clean_start(server.clean_session)
                .set_transport(transport)
                .set_connection_timeout(CONNECTION_TIMEOUT_SECS)
                .set_max_packet_size(Some(64 * 1024 * 1024))
                .set_outgoing_inflight_upper_limit(MAX_OUTGOING_INFLIGHT);
            if let Some((user, pass)) = credentials {
                options.set_credentials(user, pass);
            }
            let (client, eventloop) = rumqttc::v5::AsyncClient::new(options, REQUEST_CHANNEL_CAPACITY);
            let loop_client = client.clone();
            let flag = connected.clone();
            self.runtime.spawn(async move {
                run_v5(inner, server_id, generation, eventloop, loop_client, flag, shutdown_rx).await;
            });
            Client::V5(client)
        } else {
            let mut options = rumqttc::MqttOptions::new(client_id, host, port);
            options
                .set_keep_alive(keep_alive)
                .set_clean_session(server.clean_session)
                .set_transport(transport)
                .set_max_packet_size(64 * 1024 * 1024, 64 * 1024 * 1024);
            if let Some((user, pass)) = credentials {
                options.set_credentials(user, pass);
            }
            let (client, mut eventloop) = rumqttc::AsyncClient::new(options, REQUEST_CHANNEL_CAPACITY);
            let mut network = rumqttc::NetworkOptions::new();
            network.set_connection_timeout(CONNECTION_TIMEOUT_SECS);
            eventloop.set_network_options(network);
            let loop_client = client.clone();
            let flag = connected.clone();
            self.runtime.spawn(async move {
                run_v4(inner, server_id, generation, eventloop, loop_client, flag, shutdown_rx).await;
            });
            Client::V4(client)
        };

        self.inner.clients.lock().insert(
            server_id,
            Handle {
                client,
                generation,
                connected,
                shutdown: Some(shutdown_tx),
            },
        );
        self.inner
            .emit_status(server_id, generation, ConnectionStatus::Connecting);
        Ok(())
    }

    /// Stop the connection loop of a server without emitting any event.
    fn shutdown(&self, server_id: i64) -> bool {
        let handle = self.inner.clients.lock().remove(&server_id);
        match handle {
            Some(mut handle) => {
                if let Some(tx) = handle.shutdown.take() {
                    let _ = tx.send(());
                }
                true
            }
            None => false,
        }
    }

    pub fn disconnect(&self, server_id: i64) {
        self.shutdown(server_id);
        let _ = self.inner.events.unbounded_send(MqttEvent::Status {
            server_id,
            status: ConnectionStatus::Disconnected,
        });
    }

    pub fn disconnect_all(&self) {
        let ids: Vec<i64> = self.inner.clients.lock().keys().copied().collect();
        for id in ids {
            self.shutdown(id);
        }
    }

    /// Take up to `max` queued received messages, oldest first.
    pub fn take_messages(&self, max: usize) -> Vec<IncomingMessage> {
        let mut queue = self.inner.queue.lock();
        let n = max.min(queue.messages.len());
        let taken: Vec<IncomingMessage> = queue.messages.drain(..n).collect();
        queue.bytes -= taken.iter().map(|m| m.payload.len()).sum::<usize>();
        taken
    }

    /// Number of received messages waiting for the UI.
    pub fn queued_messages(&self) -> usize {
        self.inner.queue.lock().messages.len()
    }

    /// Number of messages dropped because the UI backlog was full, since the
    /// last call.
    pub fn take_dropped(&self) -> u64 {
        self.inner.dropped.swap(0, Ordering::Relaxed)
    }

    pub fn is_connected(&self, server_id: i64) -> bool {
        self.inner
            .clients
            .lock()
            .get(&server_id)
            .is_some_and(|h| h.connected.load(Ordering::Relaxed))
    }

    fn with_client<R>(&self, server_id: i64, f: impl FnOnce(&Client) -> Result<R, String>) -> Result<R, String> {
        let clients = self.inner.clients.lock();
        match clients.get(&server_id) {
            Some(h) if h.connected.load(Ordering::Relaxed) => f(&h.client),
            _ => Err(tr!("mqtt.not_connected").to_string()),
        }
    }

    pub fn publish(&self, server_id: i64, topic: &str, payload: Vec<u8>, qos: u8, retain: bool) -> Result<(), String> {
        self.with_client(server_id, |c| c.publish(topic.to_string(), qos, retain, payload))
    }

    pub fn subscribe(&self, server_id: i64, topic: &str, qos: u8) -> Result<(), String> {
        self.with_client(server_id, |c| c.subscribe(topic.to_string(), qos))
    }

    pub fn unsubscribe(&self, server_id: i64, topic: &str) -> Result<(), String> {
        self.with_client(server_id, |c| c.unsubscribe(topic.to_string()))
    }
}

impl Drop for MqttManager {
    fn drop(&mut self) {
        self.disconnect_all();
    }
}

/// Keep-alive interval. 0 disables it in MQTT 3.1.1; rumqttc's v5 client
/// requires at least 5 s (it panics otherwise), so 0 maps to the maximum there.
fn keep_alive_for(server: &MqttServer) -> Duration {
    let secs = server.keep_alive.clamp(0, 65535) as u64;
    if server.is_v5() {
        Duration::from_secs(if secs == 0 { 65535 } else { secs.max(5) })
    } else {
        Duration::from_secs(secs)
    }
}

/// For WebSocket transports rumqttc expects the full URL as the host. A path
/// may be given in the host field (`broker/mqtt`), otherwise `/mqtt` is used.
fn broker_host(server: &MqttServer) -> String {
    let host = server.host.trim();
    let host = host.split_once("://").map(|(_, rest)| rest).unwrap_or(host);
    match server.transport() {
        Transport::Ws | Transport::Wss => {
            let (name, path) = match host.find('/') {
                Some(idx) => (&host[..idx], &host[idx..]),
                None => (host, "/mqtt"),
            };
            format!("{}://{}:{}{}", server.transport().scheme(), name, server.port, path)
        }
        _ => host.split('/').next().unwrap_or(host).to_string(),
    }
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn build_transport(server: &MqttServer) -> Result<RumqttTransport, String> {
    Ok(match server.transport() {
        Transport::Mqtt => RumqttTransport::tcp(),
        Transport::Ws => RumqttTransport::Ws,
        Transport::Mqtts => RumqttTransport::Tls(build_tls(server)?),
        Transport::Wss => RumqttTransport::Wss(build_tls(server)?),
    })
}

fn build_tls(server: &MqttServer) -> Result<TlsConfiguration, String> {
    let mut roots = rustls::RootCertStore::empty();
    if let Some(ca) = non_empty(&server.ca_cert) {
        let certs = rustls_pemfile::certs(&mut ca.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| tr!("tls.ca_parse_failed", error = e).to_string())?;
        if certs.is_empty() {
            return Err(tr!("tls.ca_parse_failed", error = tr!("tls.no_pem_cert")).to_string());
        }
        for cert in certs {
            roots
                .add(cert)
                .map_err(|e| tr!("tls.ca_invalid", error = e).to_string())?;
        }
    } else {
        let native = rustls_native_certs::load_native_certs();
        for cert in native.certs {
            let _ = roots.add(cert);
        }
        if roots.is_empty() {
            return Err(tr!("tls.no_system_roots").to_string());
        }
    }

    let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
    let config = match (non_empty(&server.client_cert), non_empty(&server.client_key)) {
        (Some(cert), Some(key)) => {
            let chain = rustls_pemfile::certs(&mut cert.as_bytes())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| tr!("tls.cert_parse_failed", error = e).to_string())?;
            let key = rustls_pemfile::private_key(&mut key.as_bytes())
                .map_err(|e| tr!("tls.key_parse_failed", error = e).to_string())?
                .ok_or_else(|| tr!("tls.key_parse_failed", error = tr!("tls.no_pem_key")).to_string())?;
            builder
                .with_client_auth_cert(chain, key)
                .map_err(|e| tr!("tls.cert_invalid", error = e).to_string())?
        }
        (None, None) => builder.with_no_client_auth(),
        _ => return Err(tr!("tls.cert_and_key_required").to_string()),
    };
    Ok(TlsConfiguration::Rustls(Arc::new(config)))
}

fn incoming(server_id: i64, topic: String, payload: &[u8], qos: u8, retain: bool) -> IncomingMessage {
    IncomingMessage {
        server_id,
        topic,
        payload: payload.to_vec(),
        qos,
        retain,
        timestamp: Local::now(),
    }
}

async fn run_v4(
    inner: Arc<Inner>,
    server_id: i64,
    generation: u64,
    mut eventloop: rumqttc::EventLoop,
    client: rumqttc::AsyncClient,
    connected: Arc<AtomicBool>,
    mut shutdown: oneshot::Receiver<()>,
) {
    use rumqttc::{ConnectReturnCode, Event, Outgoing, Packet, SubscribeReasonCode};
    let mut was_connected = false;
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                let _ = client.try_disconnect();
                let _ = tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        match eventloop.poll().await {
                            Ok(Event::Outgoing(Outgoing::Disconnect)) | Err(_) => break,
                            _ => {}
                        }
                    }
                }).await;
                break;
            }
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                    if ack.code == ConnectReturnCode::Success {
                        was_connected = true;
                        connected.store(true, Ordering::Relaxed);
                        inner.emit_status(server_id, generation, ConnectionStatus::Connected);
                    } else {
                        inner.emit_status(server_id, generation, ConnectionStatus::Error(format!("Connection refused: {:?}", ack.code)));
                        break;
                    }
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    inner.emit_message(server_id, generation, incoming(server_id, p.topic.clone(), &p.payload, p.qos as u8, p.retain));
                }
                Ok(Event::Incoming(Packet::SubAck(ack))) => {
                    if ack.return_codes.iter().any(|c| matches!(c, SubscribeReasonCode::Failure)) {
                        inner.emit(server_id, generation, MqttEvent::Warning { server_id, message: tr!("mqtt.subscribe_rejected").to_string() });
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    let prefix = if was_connected { "Connection error" } else { "Failed to connect" };
                    inner.emit_status(server_id, generation, ConnectionStatus::Error(format!("{prefix}: {err}")));
                    break;
                }
            }
        }
    }
    connected.store(false, Ordering::Relaxed);
    inner.remove_if_current(server_id, generation);
}

async fn run_v5(
    inner: Arc<Inner>,
    server_id: i64,
    generation: u64,
    mut eventloop: rumqttc::v5::EventLoop,
    client: rumqttc::v5::AsyncClient,
    connected: Arc<AtomicBool>,
    mut shutdown: oneshot::Receiver<()>,
) {
    use rumqttc::Outgoing;
    use rumqttc::v5::Event;
    use rumqttc::v5::mqttbytes::v5::{ConnectReturnCode, Packet, SubscribeReasonCode};
    let mut was_connected = false;
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                let _ = client.try_disconnect();
                let _ = tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        match eventloop.poll().await {
                            Ok(Event::Outgoing(Outgoing::Disconnect)) | Err(_) => break,
                            _ => {}
                        }
                    }
                }).await;
                break;
            }
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                    if ack.code == ConnectReturnCode::Success {
                        was_connected = true;
                        connected.store(true, Ordering::Relaxed);
                        inner.emit_status(server_id, generation, ConnectionStatus::Connected);
                    } else {
                        inner.emit_status(server_id, generation, ConnectionStatus::Error(format!("Connection refused: {:?}", ack.code)));
                        break;
                    }
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    let topic = String::from_utf8_lossy(&p.topic).into_owned();
                    inner.emit_message(server_id, generation, incoming(server_id, topic, &p.payload, p.qos as u8, p.retain));
                }
                Ok(Event::Incoming(Packet::SubAck(ack))) => {
                    if let Some(code) = ack.return_codes.iter().find(|c| !matches!(c, SubscribeReasonCode::Success(_))) {
                        inner.emit(server_id, generation, MqttEvent::Warning { server_id, message: format!("{}: {code:?}", tr!("mqtt.subscribe_rejected")) });
                    }
                }
                Ok(Event::Incoming(Packet::Disconnect(d))) => {
                    inner.emit_status(server_id, generation, ConnectionStatus::Error(format!("{}: {:?}", tr!("mqtt.server_disconnected"), d.reason_code)));
                    break;
                }
                Ok(_) => {}
                Err(err) => {
                    let prefix = if was_connected { "Connection error" } else { "Failed to connect" };
                    inner.emit_status(server_id, generation, ConnectionStatus::Error(format!("{prefix}: {err}")));
                    break;
                }
            }
        }
    }
    connected.store(false, Ordering::Relaxed);
    inner.remove_if_current(server_id, generation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_queue_evicts_oldest_and_counts_drops() {
        let (manager, mut events) = MqttManager::new();
        manager.inner.clients.lock().insert(
            7,
            Handle {
                client: Client::V4(rumqttc::AsyncClient::new(rumqttc::MqttOptions::new("t", "localhost", 1), 1).0),
                generation: 1,
                connected: Arc::new(AtomicBool::new(true)),
                shutdown: None,
            },
        );
        for i in 0..(MAX_PENDING_MESSAGES + 10) {
            manager
                .inner
                .emit_message(7, 1, incoming(7, format!("t/{i}"), b"x", 0, false));
        }
        // Only the first push (empty -> non-empty) wakes the UI.
        assert!(matches!(events.try_recv(), Ok(MqttEvent::MessagesReady)));
        assert!(events.try_recv().is_err());
        assert_eq!(manager.queued_messages(), MAX_PENDING_MESSAGES);
        assert_eq!(manager.take_dropped(), 10);
        assert_eq!(manager.take_dropped(), 0);
        let first = manager.take_messages(2);
        assert_eq!(first[0].topic, "t/10", "oldest messages were evicted");
        // Stale generations are ignored.
        manager
            .inner
            .emit_message(7, 0, incoming(7, "stale".into(), b"x", 0, false));
        assert_eq!(manager.queued_messages(), MAX_PENDING_MESSAGES - 2);
        let rest = manager.take_messages(usize::MAX);
        assert_eq!(rest.last().unwrap().topic, format!("t/{}", MAX_PENDING_MESSAGES + 9));
        // Draining re-arms the wake-up.
        manager
            .inner
            .emit_message(7, 1, incoming(7, "again".into(), b"x", 0, false));
        assert!(matches!(events.try_recv(), Ok(MqttEvent::MessagesReady)));
    }

    #[test]
    fn keep_alive_respects_protocol_limits() {
        let mut server = MqttServer {
            keep_alive: 0,
            protocol_version: "3.1.1".into(),
            ..Default::default()
        };
        assert_eq!(keep_alive_for(&server), Duration::ZERO);
        server.keep_alive = 2;
        assert_eq!(keep_alive_for(&server), Duration::from_secs(2));
        server.protocol_version = "5.0".into();
        assert_eq!(keep_alive_for(&server), Duration::from_secs(5));
        server.keep_alive = 0;
        assert_eq!(keep_alive_for(&server), Duration::from_secs(65535));
        // Constructing options must not panic for any of these values.
        for ka in [0, 1, 4, 5, 60, 65535] {
            server.keep_alive = ka;
            let mut v5 = rumqttc::v5::MqttOptions::new("id", "localhost", 1883);
            v5.set_keep_alive(keep_alive_for(&server));
            let mut v4 = rumqttc::MqttOptions::new("id", "localhost", 1883);
            server.protocol_version = "3.1.1".into();
            v4.set_keep_alive(keep_alive_for(&server));
            server.protocol_version = "5.0".into();
        }
    }

    #[test]
    fn websocket_host_gets_url_and_default_path() {
        let mut server = MqttServer {
            host: "broker.local".into(),
            port: 8083,
            protocol: Some("ws".into()),
            ..Default::default()
        };
        assert_eq!(broker_host(&server), "ws://broker.local:8083/mqtt");
        server.host = "broker.local/custom".into();
        assert_eq!(broker_host(&server), "ws://broker.local:8083/custom");
        server.protocol = Some("mqtt".into());
        server.host = "mqtt://broker.local".into();
        assert_eq!(broker_host(&server), "broker.local");
    }

    /// End-to-end against a real broker. Run with e.g.
    /// `MQTT_TEST_BROKER=127.0.0.1 MQTT_TEST_WS_PORT=8083 cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn roundtrip_against_live_broker() {
        use futures::StreamExt;
        let host = std::env::var("MQTT_TEST_BROKER").unwrap_or_else(|_| "127.0.0.1".into());
        let ws_port: i32 = std::env::var("MQTT_TEST_WS_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8083);
        let (manager, mut events) = MqttManager::new();
        let mut next = move || {
            futures::executor::block_on(async {
                tokio::time::timeout(Duration::from_secs(10), events.next())
                    .await
                    .expect("timed out waiting for event")
                    .expect("event channel closed")
            })
        };
        let _rt = manager.runtime.enter();
        let cases = [
            ("mqtt", "3.1.1", 1883),
            ("mqtt", "5.0", 1883),
            ("ws", "3.1.1", ws_port),
            ("ws", "5.0", ws_port),
        ];
        for (i, (protocol, version, port)) in cases.into_iter().enumerate() {
            let id = i as i64 + 1;
            let server = MqttServer {
                id: Some(id),
                host: host.clone(),
                port,
                protocol: Some(protocol.into()),
                protocol_version: version.into(),
                ..Default::default()
            };
            manager.connect(&server).unwrap();
            assert!(matches!(
                next(),
                MqttEvent::Status {
                    status: ConnectionStatus::Connecting,
                    ..
                }
            ));
            match next() {
                MqttEvent::Status {
                    status: ConnectionStatus::Connected,
                    server_id,
                } => assert_eq!(server_id, id),
                other => panic!("{protocol} {version}: expected connected, got {other:?}"),
            }
            let topic = format!("mini-mqtt-test/{protocol}/{version}");
            manager.subscribe(id, &topic, 1).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            manager.publish(id, &topic, b"hello".to_vec(), 1, false).unwrap();
            match next() {
                MqttEvent::MessagesReady => {
                    let taken = manager.take_messages(10);
                    assert_eq!(taken.len(), 1);
                    let m = &taken[0];
                    assert_eq!(m.topic, topic);
                    assert_eq!(m.payload, b"hello");
                    assert_eq!(m.qos, 1);
                }
                other => panic!("expected message, got {other:?}"),
            }
            manager.disconnect(id);
            assert!(matches!(
                next(),
                MqttEvent::Status {
                    status: ConnectionStatus::Disconnected,
                    ..
                }
            ));
            assert!(manager.publish(id, &topic, vec![], 0, false).is_err());
        }

        // TLS with a custom CA (MQTT_TEST_CA = path to a PEM file).
        if let Ok(ca_path) = std::env::var("MQTT_TEST_CA") {
            let ca = std::fs::read_to_string(ca_path).unwrap();
            for (id, version) in [(50, "3.1.1"), (51, "5.0")] {
                let server = MqttServer {
                    id: Some(id),
                    host: "localhost".into(),
                    port: 8883,
                    protocol: Some("mqtts".into()),
                    protocol_version: version.into(),
                    ca_cert: Some(ca.clone()),
                    use_tls: true,
                    ..Default::default()
                };
                manager.connect(&server).unwrap();
                assert!(matches!(
                    next(),
                    MqttEvent::Status {
                        status: ConnectionStatus::Connecting,
                        ..
                    }
                ));
                match next() {
                    MqttEvent::Status {
                        status: ConnectionStatus::Connected,
                        ..
                    } => {}
                    other => panic!("tls {version}: expected connected, got {other:?}"),
                }
                manager.disconnect(id);
                assert!(matches!(
                    next(),
                    MqttEvent::Status {
                        status: ConnectionStatus::Disconnected,
                        ..
                    }
                ));
            }
        }

        // Unreachable port reports an error state.
        let bad = MqttServer {
            id: Some(99),
            host: host.clone(),
            port: 1,
            ..Default::default()
        };
        manager.connect(&bad).unwrap();
        assert!(matches!(
            next(),
            MqttEvent::Status {
                status: ConnectionStatus::Connecting,
                ..
            }
        ));
        match next() {
            MqttEvent::Status {
                status: ConnectionStatus::Error(e),
                ..
            } => {
                assert!(crate::backend::errors::describe_mqtt_error(&e).is_some(), "{e}")
            }
            other => panic!("expected error, got {other:?}"),
        }
    }
}
