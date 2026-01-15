pub mod models;

use models::{CommandTemplate, CreateTemplateRequest, MessageHistory, MqttServer, Subscription, UpdateTemplateRequest};
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
    pub templates: Vec<CommandTemplate>,
    #[serde(default)]
    next_server_id: i64,
    #[serde(default)]
    next_subscription_id: i64,
    #[serde(default)]
    next_message_id: i64,
    #[serde(default)]
    next_template_id: i64,
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

    // ===== 模板操作 =====
    pub fn get_templates(&self, server_id: i64) -> Vec<CommandTemplate> {
        let data = self.data.read();
        data.templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .cloned()
            .collect()
    }

    pub fn get_template(&self, id: i64) -> Option<CommandTemplate> {
        let data = self.data.read();
        data.templates.iter().find(|t| t.id == Some(id)).cloned()
    }

    pub fn create_template(&self, req: CreateTemplateRequest) -> Result<i64, String> {
        let mut data = self.data.write();
        data.next_template_id += 1;
        let id = data.next_template_id;
        let now = chrono::Utc::now().to_rfc3339();
        
        let template = CommandTemplate {
            id: Some(id),
            server_id: req.server_id,
            name: req.name,
            topic: req.topic,
            payload: req.payload,
            payload_type: req.payload_type,
            qos: req.qos,
            retain: req.retain,
            description: req.description,
            category: req.category,
            use_count: 0,
            last_used_at: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };
        
        data.templates.push(template);
        drop(data);
        self.save()?;
        Ok(id)
    }

    pub fn update_template(&self, req: UpdateTemplateRequest) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(template) = data.templates.iter_mut().find(|t| t.id == Some(req.id)) {
            if let Some(name) = req.name {
                template.name = name;
            }
            if let Some(topic) = req.topic {
                template.topic = topic;
            }
            if let Some(payload) = req.payload {
                template.payload = payload;
            }
            if let Some(payload_type) = req.payload_type {
                template.payload_type = payload_type;
            }
            if let Some(qos) = req.qos {
                template.qos = qos;
            }
            if let Some(retain) = req.retain {
                template.retain = retain;
            }
            if let Some(description) = req.description {
                template.description = Some(description);
            }
            if let Some(category) = req.category {
                template.category = Some(category);
            }
            template.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.save()
    }

    pub fn delete_template(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.templates.retain(|t| t.id != Some(id));
        drop(data);
        self.save()
    }

    pub fn increment_template_use_count(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(template) = data.templates.iter_mut().find(|t| t.id == Some(id)) {
            template.use_count += 1;
            template.last_used_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.save()
    }

    pub fn get_template_categories(&self, server_id: i64) -> Vec<String> {
        let data = self.data.read();
        let mut categories: Vec<String> = data
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .filter_map(|t| t.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }
}
