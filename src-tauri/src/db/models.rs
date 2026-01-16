use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttServer {
    pub id: Option<i64>,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub protocol_version: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub client_id: Option<String>,
    pub keep_alive: i32,
    pub clean_session: bool,
    pub use_tls: bool,
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
    pub client_key_password: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Option<i64>,
    pub server_id: i64,
    pub topic: String,
    pub qos: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub created_at: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    pub id: Option<i64>,
    pub server_id: i64,
    pub direction: String, // "publish" or "receive"
    pub topic: String,
    pub payload: Option<String>,
    #[serde(default)]
    pub payload_format: Option<String>, // "text", "json", "hex"
    pub qos: i32,
    pub retain: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPayload {
    pub topic: String,
    pub payload: String,
    pub qos: i32,
    pub retain: bool,
    pub format: String, // "json" | "hex" | "text"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub topic: String,
    pub payload: String,
    pub payload_type: String, // "json" | "hex" | "text"
    pub qos: i32,
    pub retain: bool,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub use_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTemplateRequest {
    pub server_id: i64,
    pub name: String,
    pub topic: String,
    pub payload: String,
    pub payload_type: String,
    pub qos: i32,
    pub retain: bool,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTemplateRequest {
    pub id: i64,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub payload: Option<String>,
    pub payload_type: Option<String>,
    pub qos: Option<i32>,
    pub retain: Option<bool>,
    pub description: Option<String>,
    pub category: Option<String>,
}

/// 预处理脚本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub script_type: String, // "before_publish" | "after_receive"
    pub code: String,
    #[serde(default)]
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScriptRequest {
    pub server_id: i64,
    pub name: String,
    pub script_type: String,
    pub code: String,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateScriptRequest {
    pub id: i64,
    pub name: Option<String>,
    pub code: Option<String>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
}
