//! Persistent data models.
//!
//! Field names and serde attributes are kept compatible with the `data.yaml`
//! written by the previous Tauri version, so existing user data loads as-is.
//! Every entity also keeps the keys it does not model ([`Extra`]), so saving
//! never drops data written by another version.

use serde::{Deserialize, Serialize};

/// Keys of a stored entity that this version does not model (written by an
/// older or newer version). They are written back unchanged on save.
pub type Extra = serde_yaml::Mapping;

fn default_true() -> bool {
    true
}

fn default_port() -> i32 {
    1883
}

fn default_keep_alive() -> i32 {
    60
}

fn default_protocol_version() -> String {
    "5.0".to_string()
}

/// Transport scheme used to reach the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Mqtt,
    Mqtts,
    Ws,
    Wss,
}

impl Transport {
    pub const ALL: [Transport; 4] = [Transport::Mqtt, Transport::Mqtts, Transport::Ws, Transport::Wss];

    pub fn scheme(self) -> &'static str {
        match self {
            Transport::Mqtt => "mqtt",
            Transport::Mqtts => "mqtts",
            Transport::Ws => "ws",
            Transport::Wss => "wss",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.scheme() == s)
    }

    pub fn uses_tls(self) -> bool {
        matches!(self, Transport::Mqtts | Transport::Wss)
    }

    pub fn default_port(self) -> i32 {
        match self {
            Transport::Mqtt => 1883,
            Transport::Mqtts => 8883,
            Transport::Ws => 8083,
            Transport::Wss => 8084,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MqttServer {
    pub id: Option<i64>,
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    /// `mqtt` | `mqtts` | `ws` | `wss`. Older data has no value; it is then
    /// derived from `use_tls`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub client_id: Option<String>,
    #[serde(default = "default_keep_alive")]
    pub keep_alive: i32,
    #[serde(default = "default_true")]
    pub clean_session: bool,
    #[serde(default)]
    pub use_tls: bool,
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
    /// Password of an encrypted (PKCS#8) client key.
    #[serde(default)]
    pub client_key_password: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for MqttServer {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            host: String::new(),
            port: 1883,
            protocol_version: default_protocol_version(),
            protocol: Some("mqtt".into()),
            username: None,
            password: None,
            client_id: None,
            keep_alive: 60,
            clean_session: true,
            use_tls: false,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            client_key_password: None,
            created_at: None,
            updated_at: None,
            extra: Extra::new(),
        }
    }
}

impl MqttServer {
    pub fn transport(&self) -> Transport {
        self.protocol
            .as_deref()
            .and_then(Transport::parse)
            .unwrap_or(if self.use_tls {
                Transport::Mqtts
            } else {
                Transport::Mqtt
            })
    }

    pub fn is_v5(&self) -> bool {
        self.protocol_version.trim() == "5.0" || self.protocol_version.trim() == "5"
    }

    /// `scheme://host:port`, as shown in the sidebar and header.
    pub fn address(&self) -> String {
        format!("{}://{}:{}", self.transport().scheme(), self.host, self.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub id: Option<i64>,
    pub server_id: i64,
    pub topic: String,
    pub qos: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
    /// Colour mark of the subscription (`#RRGGBB` or `rgba(..)`), used to
    /// highlight its messages in the list.
    #[serde(default)]
    pub color: Option<String>,
    pub created_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Message history kept from the previous version. It is preserved on disk
/// but no longer written: the message list is an in-memory live view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    pub id: Option<i64>,
    pub server_id: i64,
    pub direction: String,
    pub topic: String,
    pub payload: Option<String>,
    #[serde(default)]
    pub payload_format: Option<String>,
    pub qos: i32,
    pub retain: bool,
    pub created_at: Option<String>,
}

/// Payload encoding of a message being published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PayloadFormat {
    #[default]
    Json,
    Hex,
    Text,
}

impl PayloadFormat {
    pub const ALL: [PayloadFormat; 3] = [PayloadFormat::Json, PayloadFormat::Text, PayloadFormat::Hex];

    pub fn as_str(self) -> &'static str {
        match self {
            PayloadFormat::Json => "json",
            PayloadFormat::Hex => "hex",
            PayloadFormat::Text => "text",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PayloadFormat::Json => "JSON",
            PayloadFormat::Hex => "HEX",
            PayloadFormat::Text => "TEXT",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "hex" | "binary" => PayloadFormat::Hex,
            "text" => PayloadFormat::Text,
            _ => PayloadFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandTemplate {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub topic: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default = "default_payload_type")]
    pub payload_type: String,
    #[serde(default)]
    pub qos: i32,
    #[serde(default)]
    pub retain: bool,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub use_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

fn default_payload_type() -> String {
    "json".to_string()
}

impl CommandTemplate {
    pub fn format(&self) -> PayloadFormat {
        PayloadFormat::parse(&self.payload_type)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TemplateDraft {
    pub name: String,
    pub topic: String,
    pub payload: String,
    pub payload_type: PayloadFormat,
    pub qos: i32,
    pub retain: bool,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    BeforePublish,
    AfterReceive,
}

impl ScriptType {
    pub fn as_str(self) -> &'static str {
        match self {
            ScriptType::BeforePublish => "before_publish",
            ScriptType::AfterReceive => "after_receive",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "after_receive" {
            ScriptType::AfterReceive
        } else {
            ScriptType::BeforePublish
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ScriptType::BeforePublish => "发送前",
            ScriptType::AfterReceive => "接收后",
        }
    }
}

/// Pre-processing script (JavaScript).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub script_type: String,
    pub code: String,
    #[serde(default)]
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Script {
    pub fn kind(&self) -> ScriptType {
        ScriptType::parse(&self.script_type)
    }
}

/// Environment variable of a server, referenced as `{{name}}` in topics and
/// payloads and as `env.name` in scripts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvVariable {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_server_yaml_loads() {
        // Shape written by the Tauri version (no `protocol` field).
        let yaml = r#"
id: 3
name: test
host: broker.local
port: 8883
protocol_version: '5.0'
username: ''
password: ''
client_id: ''
keep_alive: 60
clean_session: true
use_tls: true
ca_cert: ''
client_cert: ''
client_key: ''
created_at: 2024-01-01T00:00:00Z
updated_at: 2024-01-01T00:00:00Z
"#;
        let server: MqttServer = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(server.transport(), Transport::Mqtts);
        assert!(server.is_v5());
        assert_eq!(server.address(), "mqtts://broker.local:8883");
    }
}
