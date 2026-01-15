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
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub id: Option<i64>,
    pub server_id: i64,
    pub name: String,
    pub topic: String,
    pub payload: Option<String>,
    pub qos: i32,
    pub retain: bool,
    pub category: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    pub id: Option<i64>,
    pub server_id: i64,
    pub direction: String, // "publish" or "receive"
    pub topic: String,
    pub payload: Option<Vec<u8>>,
    pub qos: i32,
    pub retain: bool,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Option<i64>,
    pub server_id: i64,
    pub topic: String,
    pub qos: i32,
    pub created_at: Option<String>,
}
