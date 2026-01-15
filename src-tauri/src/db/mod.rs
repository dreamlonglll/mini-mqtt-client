pub mod models;

use models::{MessageHistory, MqttServer, Subscription};
use parking_lot::RwLock;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct AppData {
    pub servers: Vec<MqttServer>,
    pub subscriptions: Vec<Subscription>,
    pub messages: Vec<MessageHistory>,
    #[serde(default)]
    next_server_id: i64,
    #[serde(default)]
    next_subscription_id: i64,
    #[serde(default)]
    next_message_id: i64,
}

pub struct Storage {
    data: RwLock<AppData>,
    file_path: PathBuf,
}

impl Storage {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;

        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

        let file_path = app_dir.join("data.yaml");

        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
            serde_yaml::from_str(&content).unwrap_or_default()
        } else {
            AppData::default()
        };

        Ok(Self {
            data: RwLock::new(data),
            file_path,
        })
    }

    fn save(&self) -> Result<(), String> {
        let data = self.data.read();
        let content = serde_yaml::to_string(&*data).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, content).map_err(|e| e.to_string())
    }

    // ===== Server 操作 =====
    pub fn get_servers(&self) -> Vec<MqttServer> {
        let data = self.data.read();
        data.servers.clone()
    }

    pub fn get_server(&self, id: i64) -> Option<MqttServer> {
        let data = self.data.read();
        data.servers.iter().find(|s| s.id == Some(id)).cloned()
    }

    pub fn create_server(&self, mut server: MqttServer) -> Result<i64, String> {
        let mut data = self.data.write();
        data.next_server_id += 1;
        let id = data.next_server_id;
        server.id = Some(id);
        server.created_at = Some(chrono::Utc::now().to_rfc3339());
        server.updated_at = server.created_at.clone();
        data.servers.push(server);
        drop(data);
        self.save()?;
        Ok(id)
    }

    pub fn update_server(&self, server: MqttServer) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(existing) = data.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
            existing.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.save()
    }

    pub fn delete_server(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.servers.retain(|s| s.id != Some(id));
        // 同时删除相关订阅和消息
        data.subscriptions.retain(|s| s.server_id != id);
        data.messages.retain(|m| m.server_id != id);
        drop(data);
        self.save()
    }

    // ===== 订阅操作 =====
    pub fn get_subscriptions(&self, server_id: i64) -> Vec<Subscription> {
        let data = self.data.read();
        data.subscriptions
            .iter()
            .filter(|s| s.server_id == server_id)
            .cloned()
            .collect()
    }

    pub fn create_subscription(&self, mut sub: Subscription) -> Result<Subscription, String> {
        let mut data = self.data.write();
        data.next_subscription_id += 1;
        sub.id = Some(data.next_subscription_id);
        sub.created_at = Some(chrono::Utc::now().to_rfc3339());
        let result = sub.clone();
        data.subscriptions.push(sub);
        drop(data);
        self.save()?;
        Ok(result)
    }

    pub fn update_subscription_status(&self, id: i64, is_active: bool) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(sub) = data.subscriptions.iter_mut().find(|s| s.id == Some(id)) {
            sub.is_active = is_active;
        }
        drop(data);
        self.save()
    }

    pub fn delete_subscription(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.subscriptions.retain(|s| s.id != Some(id));
        drop(data);
        self.save()
    }

    // ===== 消息操作 =====
    pub fn get_messages(&self, server_id: i64, limit: usize) -> Vec<MessageHistory> {
        let data = self.data.read();
        data.messages
            .iter()
            .filter(|m| m.server_id == server_id)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn create_message(&self, mut msg: MessageHistory) -> Result<MessageHistory, String> {
        let mut data = self.data.write();
        data.next_message_id += 1;
        msg.id = Some(data.next_message_id);
        msg.created_at = Some(chrono::Utc::now().to_rfc3339());
        let result = msg.clone();
        data.messages.push(msg);

        // 限制消息数量，每个server最多保存1000条
        let server_id = result.server_id;
        let count = data
            .messages
            .iter()
            .filter(|m| m.server_id == server_id)
            .count();
        if count > 1000 {
            let to_remove = count - 1000;
            let mut removed = 0;
            data.messages.retain(|m| {
                if m.server_id == server_id && removed < to_remove {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }

        drop(data);
        self.save()?;
        Ok(result)
    }

    pub fn clear_messages(&self, server_id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.messages.retain(|m| m.server_id != server_id);
        drop(data);
        self.save()
    }
}
