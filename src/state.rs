//! Application state shared by all views (the role Pinia stores played in the
//! Vue version). Owns storage, the MQTT manager and the live message lists.

use crate::backend::config::{self, AppConfig, ThemePreference};
use crate::backend::errors::friendly_mqtt_error;
use crate::backend::logger::{LogKind, LogManager};
use crate::backend::models::{
    CommandTemplate, MqttServer, PayloadFormat, Script, ScriptType, Subscription, TemplateDraft,
};
use crate::backend::mqtt::{ConnectionStatus, IncomingMessage, MqttEvent, MqttManager};
use crate::backend::payload::{self, DetectedFormat};
use crate::backend::storage::Storage;
use crate::backend::{script, topic};
use chrono::{DateTime, Local};
use futures::StreamExt;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Live messages kept per server (same limit as before)...
pub const MAX_MESSAGES: usize = 1000;
/// ...and at most this many payload bytes per server, so large payloads
/// cannot exhaust memory (the newest message is always kept).
pub const MAX_MESSAGE_BYTES: usize = 128 * 1024 * 1024;
/// Text kept for the list preview and search; the detail view decodes the
/// full payload on demand.
const PREVIEW_BYTES: usize = 4 * 1024;
/// Events handled per UI update; the pump then yields a frame to rendering.
const PUMP_BATCH: usize = 5_000;
const PUMP_FRAME: std::time::Duration = std::time::Duration::from_millis(16);
/// Messages per background task when running "after receive" scripts.
const SCRIPT_CHUNK: usize = 256;
/// Script errors written to the log per batch (the rest are counted).
const MAX_LOGGED_SCRIPT_ERRORS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Publish,
    Receive,
}

#[derive(Clone)]
pub struct Message {
    pub id: u64,
    pub direction: Direction,
    pub topic: SharedString,
    pub payload: Arc<[u8]>,
    /// Lossy UTF-8 text of the first few KB of the payload (list preview).
    pub preview: SharedString,
    /// Lower-cased topic + preview, for search.
    pub search_text: SharedString,
    pub format: DetectedFormat,
    pub qos: u8,
    pub retain: bool,
    pub timestamp: DateTime<Local>,
    pub script_error: Option<SharedString>,
}

impl Message {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: u64,
        direction: Direction,
        topic: impl Into<SharedString>,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        timestamp: DateTime<Local>,
        script_error: Option<String>,
    ) -> Self {
        let format = payload::detect_format(&payload);
        let topic: SharedString = topic.into();
        let (preview, cut) = payload::text_prefix(&payload, PREVIEW_BYTES);
        let search_text = format!("{}\n{}", topic, preview).to_lowercase();
        let preview = if cut { format!("{preview}…") } else { preview };
        Self {
            id,
            direction,
            topic,
            preview: preview.into(),
            search_text: search_text.into(),
            payload: payload.into(),
            format,
            qos,
            retain,
            timestamp,
            script_error: script_error.map(Into::into),
        }
    }

    pub fn time_label(&self) -> String {
        self.timestamp.format("%H:%M:%S%.3f").to_string()
    }

    /// Lossy UTF-8 text of the whole payload.
    pub fn full_text(&self) -> String {
        payload::bytes_to_text(&self.payload)
    }
}

/// Live message list of one server, newest first.
#[derive(Default)]
pub struct ServerMessages {
    list: VecDeque<Message>,
    bytes: usize,
}

impl ServerMessages {
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    fn push_front(&mut self, message: Message) {
        self.bytes += message.payload.len();
        self.list.push_front(message);
        while self.list.len() > MAX_MESSAGES || (self.bytes > MAX_MESSAGE_BYTES && self.list.len() > 1) {
            if let Some(old) = self.list.pop_back() {
                self.bytes -= old.payload.len();
            }
        }
    }
}

/// A received message after "after receive" scripts ran.
struct ProcessedMessage {
    incoming: IncomingMessage,
    payload: Option<Vec<u8>>,
    error: Option<String>,
}

/// Everything needed to publish one message.
#[derive(Debug, Clone)]
pub struct PublishDraft {
    pub topic: String,
    pub payload: String,
    pub format: PayloadFormat,
    pub qos: u8,
    pub retain: bool,
}

impl From<&CommandTemplate> for PublishDraft {
    fn from(t: &CommandTemplate) -> Self {
        Self {
            topic: t.topic.clone(),
            payload: t.payload.clone(),
            format: t.format(),
            qos: t.qos.clamp(0, 2) as u8,
            retain: t.retain,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
pub enum AppEvent {
    Toast(ToastKind, SharedString),
    /// The live message list of a server changed.
    MessagesChanged(i64),
    /// Fill the publish panel (from a template or "copy to publish").
    FillPublish(PublishDraft),
}

/// Blue accent of the previous (Element Plus) UI, applied on top of the
/// GPUI Kit light/dark themes.
fn apply_brand_colors(cx: &mut App) {
    let primary: Hsla = rgb(0x409eff).into();
    let hover: Hsla = rgb(0x66b1ff).into();
    let active: Hsla = rgb(0x337ecc).into();
    let white: Hsla = rgb(0xffffff).into();
    Theme::update(cx, |theme| {
        let dark = theme.mode.is_dark();
        theme.primary = primary;
        theme.primary_hover = hover;
        theme.primary_active = active;
        theme.primary_foreground = white;
        theme.button_primary = primary;
        theme.button_primary_hover = hover;
        theme.button_primary_active = active;
        theme.button_primary_foreground = white;
        theme.ring = primary;
        theme.link = primary;
        theme.link_hover = hover;
        theme.link_active = active;
        theme.caret = primary;
        theme.selection = primary.opacity(0.25);
        theme.switch = if dark { theme.switch } else { rgb(0xdcdfe6).into() };
        theme.sidebar_primary = primary;
        theme.sidebar_primary_foreground = white;
        theme.sidebar_accent = primary.opacity(if dark { 0.22 } else { 0.12 });
        theme.sidebar_accent_foreground = if dark { hover } else { active };
        theme.accent = primary.opacity(if dark { 0.18 } else { 0.08 });
        theme.progress_bar = primary;
        theme.slider_bar = primary;
        theme.slider_thumb = white;
        theme.tab_active_foreground = theme.foreground;
        theme.list_active = primary.opacity(0.1);
        theme.list_active_border = primary;
        // Top-center like the previous version's messages, so toasts do not
        // cover the search box and buttons in the top-right corner.
        theme.notification.placement = Anchor::TopCenter;
    });
}

pub struct AppState {
    pub app_dir: PathBuf,
    pub config: AppConfig,
    pub storage: Arc<Storage>,
    pub logs: Arc<LogManager>,
    pub mqtt: MqttManager,
    pub servers: Vec<MqttServer>,
    pub active_server_id: Option<i64>,
    statuses: HashMap<i64, ConnectionStatus>,
    messages: HashMap<i64, ServerMessages>,
    last_drop_notice: Option<std::time::Instant>,
    next_message_id: u64,
    _pump: Task<()>,
}

impl EventEmitter<AppEvent> for AppState {}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let app_dir = config::app_data_dir();
        let config = AppConfig::load(&app_dir);
        let logs = Arc::new(LogManager::new(&app_dir));
        let data_file = config.data_file(&app_dir);
        let storage = match Storage::open(data_file.clone()) {
            Ok(storage) => storage,
            Err(err) => {
                logs.write(
                    LogKind::Storage,
                    &format!("无法打开数据文件 {}", data_file.display()),
                    Some(&err),
                );
                let fallback = app_dir.join("data.yaml");
                Storage::open(fallback).unwrap_or_else(|e| panic!("无法初始化存储: {e}"))
            }
        };
        let (mqtt, mut events) = MqttManager::new();
        let servers = storage.servers();
        let active_server_id = servers.first().and_then(|s| s.id);

        let pump = cx.spawn(async move |this, cx| {
            // Drain the channel in batches: one UI update per batch, then a
            // frame for rendering, so a message flood cannot starve the UI.
            while let Some(first) = events.next().await {
                let mut batch = vec![first];
                while batch.len() < PUMP_BATCH {
                    match events.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(_) => break,
                    }
                }
                if Self::process_batch(&this, batch, cx).await.is_err() {
                    break;
                }
                cx.background_executor().timer(PUMP_FRAME).await;
            }
        });

        let state = Self {
            app_dir,
            config,
            storage: Arc::new(storage),
            logs,
            mqtt,
            servers,
            active_server_id,
            statuses: HashMap::new(),
            messages: HashMap::new(),
            last_drop_notice: None,
            next_message_id: 1,
            _pump: pump,
        };
        state.apply_theme(cx);
        state
    }

    /// Handle a batch of events in arrival order: runs of received messages
    /// are processed together, status changes in between are applied as they come.
    async fn process_batch(this: &WeakEntity<Self>, batch: Vec<MqttEvent>, cx: &mut AsyncApp) -> anyhow::Result<()> {
        let mut run = Vec::new();
        for event in batch {
            match event {
                MqttEvent::Message(msg) => run.push(msg),
                other => {
                    if !run.is_empty() {
                        Self::process_messages(this, std::mem::take(&mut run), cx).await?;
                    }
                    this.update(cx, |state, cx| match other {
                        MqttEvent::Status { server_id, status } => state.on_status(server_id, status, cx),
                        MqttEvent::Warning { server_id, message } => {
                            state
                                .logs
                                .write(LogKind::Mqtt, &message, Some(&format!("server_id={server_id}")));
                            state.toast(ToastKind::Warning, message, cx);
                        }
                        MqttEvent::Message(_) => unreachable!(),
                    })?;
                }
            }
        }
        if !run.is_empty() {
            Self::process_messages(this, run, cx).await?;
        }
        Ok(())
    }

    /// Run "after receive" scripts off the UI thread (in parallel chunks,
    /// order preserved), then add the messages to the lists in one update.
    async fn process_messages(
        this: &WeakEntity<Self>,
        messages: Vec<IncomingMessage>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let mut server_ids: Vec<i64> = messages.iter().map(|m| m.server_id).collect();
        server_ids.sort_unstable();
        server_ids.dedup();
        let chains: Arc<HashMap<i64, Vec<(String, String)>>> = Arc::new(this.read_with(cx, |state, _| {
            server_ids
                .iter()
                .map(|id| (*id, state.script_chain(*id, ScriptType::AfterReceive)))
                .filter(|(_, chain)| !chain.is_empty())
                .collect()
        })?);

        let processed: Vec<ProcessedMessage> = if chains.is_empty() {
            messages
                .into_iter()
                .map(|incoming| ProcessedMessage {
                    incoming,
                    payload: None,
                    error: None,
                })
                .collect()
        } else {
            let mut tasks = Vec::new();
            let mut messages = messages.into_iter().peekable();
            while messages.peek().is_some() {
                let chunk: Vec<IncomingMessage> = messages.by_ref().take(SCRIPT_CHUNK).collect();
                let chains = chains.clone();
                tasks.push(cx.background_executor().spawn(async move {
                    chunk
                        .into_iter()
                        .map(|incoming| Self::run_receive_scripts(&chains, incoming))
                        .collect::<Vec<_>>()
                }));
            }
            let mut out = Vec::new();
            for task in tasks {
                out.extend(task.await);
            }
            out
        };
        this.update(cx, |state, cx| state.push_received(processed, cx))
    }

    fn run_receive_scripts(
        chains: &HashMap<i64, Vec<(String, String)>>,
        incoming: IncomingMessage,
    ) -> ProcessedMessage {
        let Some(chain) = chains.get(&incoming.server_id) else {
            return ProcessedMessage {
                incoming,
                payload: None,
                error: None,
            };
        };
        let text = payload::bytes_to_text(&incoming.payload);
        match script::run_chain(chain, &text, &incoming.topic) {
            // Unchanged output: keep the original bytes, so binary payloads
            // are not mangled by the lossy UTF-8 round trip.
            Ok(processed) if processed == text => ProcessedMessage {
                incoming,
                payload: None,
                error: None,
            },
            Ok(processed) => ProcessedMessage {
                incoming,
                payload: Some(processed.into_bytes()),
                error: None,
            },
            Err(err) => ProcessedMessage {
                incoming,
                payload: None,
                error: Some(err.to_string()),
            },
        }
    }

    fn push_received(&mut self, processed: Vec<ProcessedMessage>, cx: &mut Context<Self>) {
        let consumed_bytes: usize = processed.iter().map(|p| p.incoming.payload.len()).sum();
        self.mqtt.messages_consumed(processed.len(), consumed_bytes);

        let mut changed: Vec<i64> = Vec::new();
        let mut script_errors = 0usize;
        for ProcessedMessage {
            incoming,
            payload,
            error,
        } in processed
        {
            if let Some(error) = &error {
                script_errors += 1;
                if script_errors <= MAX_LOGGED_SCRIPT_ERRORS {
                    self.logs
                        .write(LogKind::Script, error, Some(&format!("topic={}", incoming.topic)));
                }
            }
            if !changed.contains(&incoming.server_id) {
                changed.push(incoming.server_id);
            }
            self.insert_message(
                incoming.server_id,
                Direction::Receive,
                incoming.topic,
                payload.unwrap_or(incoming.payload),
                incoming.qos,
                incoming.retain,
                incoming.timestamp,
                error,
            );
        }
        if script_errors > MAX_LOGGED_SCRIPT_ERRORS {
            self.logs.write(
                LogKind::Script,
                &format!(
                    "另有 {} 条消息的接收后脚本执行失败",
                    script_errors - MAX_LOGGED_SCRIPT_ERRORS
                ),
                None,
            );
        }
        for server_id in changed {
            cx.emit(AppEvent::MessagesChanged(server_id));
        }

        let dropped = self.mqtt.take_dropped();
        if dropped > 0 {
            self.logs
                .write(LogKind::Mqtt, &format!("消息过多，处理不及，已丢弃 {dropped} 条"), None);
            let now = std::time::Instant::now();
            if self
                .last_drop_notice
                .is_none_or(|t| now.duration_since(t) > std::time::Duration::from_secs(5))
            {
                self.last_drop_notice = Some(now);
                self.toast(
                    ToastKind::Warning,
                    format!("消息过多，处理不及，已丢弃 {dropped} 条"),
                    cx,
                );
            }
        }
    }

    // ----------------------------------------------------------------------
    // Helpers

    pub fn toast(&self, kind: ToastKind, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        cx.emit(AppEvent::Toast(kind, message.into()));
    }

    pub fn active_server(&self) -> Option<&MqttServer> {
        let id = self.active_server_id?;
        self.servers.iter().find(|s| s.id == Some(id))
    }

    pub fn server(&self, id: i64) -> Option<&MqttServer> {
        self.servers.iter().find(|s| s.id == Some(id))
    }

    pub fn status(&self, server_id: i64) -> ConnectionStatus {
        self.statuses.get(&server_id).cloned().unwrap_or_default()
    }

    pub fn messages(&self, server_id: i64) -> Option<&ServerMessages> {
        self.messages.get(&server_id)
    }

    fn script_chain(&self, server_id: i64, kind: ScriptType) -> Vec<(String, String)> {
        self.storage
            .enabled_scripts(server_id, kind.as_str())
            .into_iter()
            .map(|s| (s.name, s.code))
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_message(
        &mut self,
        server_id: i64,
        direction: Direction,
        topic: impl Into<SharedString>,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        timestamp: DateTime<Local>,
        script_error: Option<String>,
    ) {
        let id = self.next_message_id;
        self.next_message_id += 1;
        let message = Message::new(id, direction, topic, payload, qos, retain, timestamp, script_error);
        self.messages.entry(server_id).or_default().push_front(message);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_message(
        &mut self,
        server_id: i64,
        direction: Direction,
        topic: impl Into<SharedString>,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        timestamp: DateTime<Local>,
        script_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.insert_message(
            server_id,
            direction,
            topic,
            payload,
            qos,
            retain,
            timestamp,
            script_error,
        );
        cx.emit(AppEvent::MessagesChanged(server_id));
    }

    pub fn clear_messages(&mut self, server_id: i64, cx: &mut Context<Self>) {
        self.messages.remove(&server_id);
        cx.emit(AppEvent::MessagesChanged(server_id));
    }

    // ----------------------------------------------------------------------
    // Theme

    pub fn is_dark(&self, cx: &App) -> bool {
        Theme::global(cx).mode.is_dark()
    }

    fn apply_theme(&self, cx: &mut App) {
        match self.config.theme {
            ThemePreference::System => Theme::sync_system_appearance(None, cx),
            ThemePreference::Light => Theme::change(ThemeMode::Light, None, cx),
            ThemePreference::Dark => Theme::change(ThemeMode::Dark, None, cx),
        }
        apply_brand_colors(cx);
    }

    pub fn set_theme(&mut self, theme: ThemePreference, cx: &mut Context<Self>) {
        self.config.theme = theme;
        self.apply_theme(cx);
        if let Err(err) = self.config.save(&self.app_dir) {
            self.toast(ToastKind::Error, err, cx);
        }
        cx.notify();
    }

    pub fn toggle_dark(&mut self, cx: &mut Context<Self>) {
        let next = if self.is_dark(cx) {
            ThemePreference::Light
        } else {
            ThemePreference::Dark
        };
        self.set_theme(next, cx);
    }

    // ----------------------------------------------------------------------
    // Servers & connections

    pub fn set_active_server(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.active_server_id != Some(id) {
            self.active_server_id = Some(id);
            cx.emit(AppEvent::MessagesChanged(id));
            cx.notify();
        }
    }

    fn reload_servers(&mut self) {
        self.servers = self.storage.servers();
        if self.active_server_id.is_none_or(|id| self.server(id).is_none()) {
            self.active_server_id = self.servers.first().and_then(|s| s.id);
        }
    }

    pub fn create_server(&mut self, server: MqttServer, cx: &mut Context<Self>) -> Result<i64, String> {
        let id = self.storage.create_server(server)?;
        self.reload_servers();
        self.active_server_id = Some(id);
        cx.emit(AppEvent::MessagesChanged(id));
        cx.notify();
        Ok(id)
    }

    pub fn update_server(&mut self, server: MqttServer, cx: &mut Context<Self>) -> Result<(), String> {
        let id = server.id.ok_or("Server ID is required")?;
        self.storage.update_server(server)?;
        self.reload_servers();
        if self.status(id).is_busy() {
            self.toast(ToastKind::Info, "配置已保存，重新连接后生效", cx);
        }
        cx.notify();
        Ok(())
    }

    pub fn delete_server(&mut self, id: i64, cx: &mut Context<Self>) -> Result<(), String> {
        self.mqtt.disconnect(id);
        self.storage.delete_server(id)?;
        self.messages.remove(&id);
        self.statuses.remove(&id);
        self.reload_servers();
        if let Some(active) = self.active_server_id {
            cx.emit(AppEvent::MessagesChanged(active));
        }
        cx.notify();
        Ok(())
    }

    pub fn duplicate_server(&mut self, id: i64, cx: &mut Context<Self>) -> Result<(), String> {
        let mut copy = self.server(id).cloned().ok_or("服务器不存在")?;
        copy.id = None;
        copy.name = format!("{} (副本)", copy.name);
        copy.client_id = None;
        self.create_server(copy, cx)?;
        Ok(())
    }

    pub fn connect(&mut self, server_id: i64, cx: &mut Context<Self>) {
        let Some(server) = self.server(server_id).cloned() else {
            return;
        };
        if let Err(err) = self.mqtt.connect(&server) {
            self.logs.write(LogKind::Mqtt, &err, Some(&server.address()));
            self.statuses.insert(server_id, ConnectionStatus::Error(err.clone()));
            self.toast(ToastKind::Error, format!("连接失败: {err}"), cx);
        }
        cx.notify();
    }

    pub fn disconnect(&mut self, server_id: i64, cx: &mut Context<Self>) {
        self.mqtt.disconnect(server_id);
        cx.notify();
    }

    fn on_status(&mut self, server_id: i64, status: ConnectionStatus, cx: &mut Context<Self>) {
        let name = self.server(server_id).map(|s| s.name.clone()).unwrap_or_default();
        match &status {
            ConnectionStatus::Connected => {
                // Re-subscribe every active subscription of this server.
                let subs = self.storage.subscriptions(server_id);
                let mut failed = Vec::new();
                for sub in subs.iter().filter(|s| s.is_active) {
                    if let Err(err) = self.mqtt.subscribe(server_id, &sub.topic, sub.qos.clamp(0, 2) as u8) {
                        failed.push(format!("{}: {err}", sub.topic));
                    }
                }
                if !failed.is_empty() {
                    let msg = format!("自动订阅失败: {}", failed.join("; "));
                    self.logs.write(LogKind::Mqtt, &msg, None);
                    self.toast(ToastKind::Warning, msg, cx);
                }
                self.toast(ToastKind::Success, format!("已连接到 {name}"), cx);
            }
            ConnectionStatus::Error(err) => {
                let address = self.server(server_id).map(|s| s.address()).unwrap_or_default();
                self.logs.write(LogKind::Mqtt, err, Some(&format!("{name} {address}")));
                self.toast(ToastKind::Error, friendly_mqtt_error(err), cx);
            }
            _ => {}
        }
        self.statuses.insert(server_id, status);
        cx.notify();
    }

    // ----------------------------------------------------------------------
    // Publishing

    /// Validate, run "before publish" scripts (off the UI thread), encode and
    /// publish. Resolves once the message was handed to the client.
    pub fn publish(&mut self, server_id: i64, draft: PublishDraft, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        if let Err(err) = topic::validate_publish_topic(&draft.topic) {
            return Task::ready(Err(err));
        }
        if let Err(err) = payload::validate_payload(&draft.payload, draft.format) {
            return Task::ready(Err(err));
        }
        if !self.mqtt.is_connected(server_id) {
            return Task::ready(Err("未连接到服务器，请先连接".into()));
        }
        let scripts = self.script_chain(server_id, ScriptType::BeforePublish);
        if scripts.is_empty() {
            // No scripts: publish right away, without an executor round trip.
            let payload = draft.payload.clone();
            return Task::ready(self.finish_publish(server_id, draft, Ok(payload), cx));
        }
        cx.spawn(async move |this, cx| {
            let processed = if scripts.is_empty() {
                Ok(draft.payload.clone())
            } else {
                let (payload, topic) = (draft.payload.clone(), draft.topic.clone());
                cx.background_executor()
                    .spawn(async move { script::run_chain(&scripts, &payload, &topic) })
                    .await
                    .map_err(|e| e.to_string())
            };
            this.update(cx, |state, cx| state.finish_publish(server_id, draft, processed, cx))
                .map_err(|e| e.to_string())?
        })
    }

    fn finish_publish(
        &mut self,
        server_id: i64,
        draft: PublishDraft,
        processed: Result<String, String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let processed = match processed {
            Ok(p) => p,
            Err(err) => {
                // Keep a record of what was attempted, flagged with the error.
                self.logs
                    .write(LogKind::Script, &err, Some(&format!("topic={}", draft.topic)));
                self.push_message(
                    server_id,
                    Direction::Publish,
                    draft.topic.clone(),
                    draft.payload.into_bytes(),
                    draft.qos,
                    draft.retain,
                    Local::now(),
                    Some(err.clone()),
                    cx,
                );
                return Err(err);
            }
        };
        let bytes = payload::encode_payload(&processed, draft.format)
            .map_err(|e| format!("脚本处理后的内容无法按 {} 编码: {e}", draft.format.label()))?;
        if let Err(err) = self
            .mqtt
            .publish(server_id, &draft.topic, bytes.clone(), draft.qos, draft.retain)
        {
            self.logs
                .write(LogKind::Mqtt, &format!("发布失败: {err}"), Some(&draft.topic));
            return Err(friendly_mqtt_error(&err));
        }
        self.push_message(
            server_id,
            Direction::Publish,
            draft.topic,
            bytes,
            draft.qos,
            draft.retain,
            Local::now(),
            None,
            cx,
        );
        Ok(())
    }

    pub fn fill_publish(&mut self, draft: PublishDraft, cx: &mut Context<Self>) {
        cx.emit(AppEvent::FillPublish(draft));
    }

    // ----------------------------------------------------------------------
    // Subscriptions

    pub fn subscriptions(&self, server_id: i64) -> Vec<Subscription> {
        self.storage.subscriptions(server_id)
    }

    pub fn add_subscription(
        &mut self,
        server_id: i64,
        topic_filter: String,
        qos: u8,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let topic_filter = topic_filter.trim().to_string();
        topic::validate_subscribe_topic(&topic_filter)?;
        self.storage
            .create_subscription(server_id, topic_filter.clone(), qos as i32)?;
        if self.mqtt.is_connected(server_id) {
            self.mqtt.subscribe(server_id, &topic_filter, qos)?;
        }
        cx.notify();
        Ok(())
    }

    pub fn remove_subscription(&mut self, sub: &Subscription, cx: &mut Context<Self>) {
        let id = sub.id.unwrap_or_default();
        if self.mqtt.is_connected(sub.server_id) && sub.is_active {
            let _ = self.mqtt.unsubscribe(sub.server_id, &sub.topic);
        }
        if let Err(err) = self.storage.delete_subscription(id) {
            self.toast(ToastKind::Error, err, cx);
        }
        cx.notify();
    }

    pub fn toggle_subscription(&mut self, sub: &Subscription, active: bool, cx: &mut Context<Self>) {
        let id = sub.id.unwrap_or_default();
        if let Err(err) = self.storage.set_subscription_active(id, active) {
            self.toast(ToastKind::Error, err, cx);
            return;
        }
        if self.mqtt.is_connected(sub.server_id) {
            let result = if active {
                self.mqtt
                    .subscribe(sub.server_id, &sub.topic, sub.qos.clamp(0, 2) as u8)
            } else {
                self.mqtt.unsubscribe(sub.server_id, &sub.topic)
            };
            if let Err(err) = result {
                self.toast(ToastKind::Error, err, cx);
            }
        }
        cx.notify();
    }

    // ----------------------------------------------------------------------
    // Templates

    pub fn templates(&self, server_id: i64) -> Vec<CommandTemplate> {
        self.storage.templates(server_id)
    }

    /// Most used templates first, then by name — for the quick command bar.
    pub fn quick_templates(&self, server_id: i64, limit: usize) -> Vec<CommandTemplate> {
        let mut templates = self.templates(server_id);
        templates.sort_by(|a, b| b.use_count.cmp(&a.use_count).then_with(|| a.name.cmp(&b.name)));
        templates.truncate(limit);
        templates
    }

    pub fn save_template(
        &mut self,
        server_id: i64,
        id: Option<i64>,
        draft: TemplateDraft,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if draft.name.trim().is_empty() {
            return Err("请输入模板名称".into());
        }
        topic::validate_publish_topic(&draft.topic)?;
        payload::validate_payload(&draft.payload, draft.payload_type)?;
        match id {
            Some(id) => self.storage.update_template(id, draft)?,
            None => {
                self.storage.create_template(server_id, draft)?;
            }
        }
        cx.notify();
        Ok(())
    }

    pub fn delete_template(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Err(err) = self.storage.delete_template(id) {
            self.toast(ToastKind::Error, err, cx);
        }
        cx.notify();
    }

    pub fn duplicate_template(&mut self, id: i64, cx: &mut Context<Self>) {
        let name = self
            .storage
            .template(id)
            .map(|t| format!("{} - 副本", t.name))
            .unwrap_or_default();
        match self.storage.duplicate_template(id, name) {
            Ok(_) => self.toast(ToastKind::Success, "模板已复制", cx),
            Err(err) => self.toast(ToastKind::Error, err, cx),
        }
        cx.notify();
    }

    /// Load a template into the publish panel and count the usage.
    pub fn use_template(&mut self, id: i64, cx: &mut Context<Self>) {
        match self.storage.use_template(id) {
            Ok(Some(t)) => {
                self.toast(ToastKind::Success, format!("已加载: {}", t.name), cx);
                cx.emit(AppEvent::FillPublish(PublishDraft::from(&t)));
            }
            Ok(None) => self.toast(ToastKind::Error, "模板不存在", cx),
            Err(err) => self.toast(ToastKind::Error, err, cx),
        }
        cx.notify();
    }

    pub fn export_templates(&mut self, server_id: i64, path: &Path, cx: &mut Context<Self>) {
        let result = self
            .storage
            .export_templates(server_id)
            .and_then(|json| std::fs::write(path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => self.toast(ToastKind::Success, format!("已导出到 {}", path.display()), cx),
            Err(err) => self.toast(ToastKind::Error, format!("导出失败: {err}"), cx),
        }
    }

    pub fn import_templates(&mut self, server_id: i64, path: &Path, cx: &mut Context<Self>) {
        let result = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|json| self.storage.import_templates(server_id, &json));
        match result {
            Ok(n) => self.toast(ToastKind::Success, format!("成功导入 {n} 个模板"), cx),
            Err(err) => self.toast(ToastKind::Error, format!("导入失败: {err}"), cx),
        }
        cx.notify();
    }

    // ----------------------------------------------------------------------
    // Scripts

    pub fn scripts(&self, server_id: i64) -> Vec<Script> {
        self.storage.scripts(server_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_script(
        &mut self,
        server_id: i64,
        id: Option<i64>,
        name: String,
        kind: ScriptType,
        code: String,
        description: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<i64, String> {
        if name.trim().is_empty() {
            return Err("请输入脚本名称".into());
        }
        if code.trim().is_empty() {
            return Err("请输入脚本代码".into());
        }
        script::validate(&code)?;
        let id = match id {
            Some(id) => {
                self.storage.update_script(id, name, code, description)?;
                id
            }
            None => self
                .storage
                .create_script(server_id, name, kind.as_str().to_string(), code, true, description)?,
        };
        cx.notify();
        Ok(id)
    }

    pub fn toggle_script(&mut self, id: i64, enabled: bool, cx: &mut Context<Self>) {
        if let Err(err) = self.storage.toggle_script(id, enabled) {
            self.toast(ToastKind::Error, err, cx);
        }
        cx.notify();
    }

    pub fn delete_script(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Err(err) = self.storage.delete_script(id) {
            self.toast(ToastKind::Error, err, cx);
        }
        cx.notify();
    }

    // ----------------------------------------------------------------------
    // Settings

    pub fn data_file(&self) -> PathBuf {
        self.storage.file_path().to_path_buf()
    }

    pub fn migrate_data_path(
        &mut self,
        new_path: PathBuf,
        copy_data: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let current = self.data_file();
        config::migrate_data_path(&self.app_dir, &mut self.config, &current, &new_path, copy_data)?;
        cx.notify();
        Ok(())
    }
}
