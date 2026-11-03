//! YAML-file storage, ported from the Tauri backend (`src-tauri/src/db`).

use super::models::{CommandTemplate, MessageHistory, MqttServer, Script, Subscription, TemplateDraft};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppData {
    #[serde(default)]
    pub servers: Vec<MqttServer>,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
    #[serde(default)]
    pub messages: Vec<MessageHistory>,
    #[serde(default)]
    pub templates: Vec<CommandTemplate>,
    #[serde(default)]
    pub scripts: Vec<Script>,
    #[serde(default)]
    next_server_id: i64,
    #[serde(default)]
    next_subscription_id: i64,
    #[serde(default)]
    next_message_id: i64,
    #[serde(default)]
    next_template_id: i64,
    #[serde(default)]
    next_script_id: i64,
}

impl AppData {
    /// Guard against hand-edited files whose counters lag behind the data.
    fn fix_counters(&mut self) {
        let max = |ids: &mut dyn Iterator<Item = Option<i64>>| ids.flatten().max().unwrap_or(0);
        self.next_server_id = self.next_server_id.max(max(&mut self.servers.iter().map(|s| s.id)));
        self.next_subscription_id = self
            .next_subscription_id
            .max(max(&mut self.subscriptions.iter().map(|s| s.id)));
        self.next_message_id = self.next_message_id.max(max(&mut self.messages.iter().map(|s| s.id)));
        self.next_template_id = self.next_template_id.max(max(&mut self.templates.iter().map(|s| s.id)));
        self.next_script_id = self.next_script_id.max(max(&mut self.scripts.iter().map(|s| s.id)));
    }
}

pub struct Storage {
    data: RwLock<AppData>,
    file_path: PathBuf,
}

impl Storage {
    pub fn open(file_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut data: AppData = if file_path.exists() {
            let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
            match serde_yaml::from_str(&content) {
                Ok(data) => data,
                Err(err) => {
                    // Never silently overwrite a file we could not parse.
                    let backup = file_path.with_extension("yaml.bak");
                    let _ = fs::copy(&file_path, &backup);
                    log::error!("data file is invalid ({err}); backed up to {}", backup.display());
                    AppData::default()
                }
            }
        } else {
            AppData::default()
        };
        data.fix_counters();
        Ok(Self {
            data: RwLock::new(data),
            file_path,
        })
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    fn save(&self) -> Result<(), String> {
        let content = {
            let data = self.data.read();
            serde_yaml::to_string(&*data).map_err(|e| e.to_string())?
        };
        // Write atomically so a crash mid-write cannot corrupt the data file.
        let tmp = self.file_path.with_extension("yaml.tmp");
        fs::write(&tmp, content).map_err(|e| format!("保存数据失败: {e}"))?;
        fs::rename(&tmp, &self.file_path).map_err(|e| format!("保存数据失败: {e}"))
    }

    // ===== Servers =====

    pub fn servers(&self) -> Vec<MqttServer> {
        self.data.read().servers.clone()
    }

    pub fn create_server(&self, mut server: MqttServer) -> Result<i64, String> {
        let id = {
            let mut data = self.data.write();
            data.next_server_id += 1;
            let id = data.next_server_id;
            server.id = Some(id);
            server.created_at = Some(now());
            server.updated_at = server.created_at.clone();
            data.servers.push(server);
            id
        };
        self.save()?;
        Ok(id)
    }

    pub fn update_server(&self, server: MqttServer) -> Result<(), String> {
        {
            let mut data = self.data.write();
            if let Some(existing) = data.servers.iter_mut().find(|s| s.id == server.id) {
                let created_at = existing.created_at.clone();
                *existing = server;
                existing.created_at = created_at;
                existing.updated_at = Some(now());
            }
        }
        self.save()
    }

    pub fn delete_server(&self, id: i64) -> Result<(), String> {
        {
            let mut data = self.data.write();
            data.servers.retain(|s| s.id != Some(id));
            data.subscriptions.retain(|s| s.server_id != id);
            data.messages.retain(|m| m.server_id != id);
            data.templates.retain(|t| t.server_id != id);
            data.scripts.retain(|s| s.server_id != id);
        }
        self.save()
    }

    // ===== Subscriptions =====

    pub fn subscriptions(&self, server_id: i64) -> Vec<Subscription> {
        self.data
            .read()
            .subscriptions
            .iter()
            .filter(|s| s.server_id == server_id)
            .cloned()
            .collect()
    }

    pub fn create_subscription(&self, server_id: i64, topic: String, qos: i32) -> Result<Subscription, String> {
        let sub = {
            let mut data = self.data.write();
            if data
                .subscriptions
                .iter()
                .any(|s| s.server_id == server_id && s.topic == topic)
            {
                return Err(format!("已存在订阅: {topic}"));
            }
            data.next_subscription_id += 1;
            let sub = Subscription {
                id: Some(data.next_subscription_id),
                server_id,
                topic,
                qos,
                is_active: true,
                created_at: Some(now()),
            };
            data.subscriptions.push(sub.clone());
            sub
        };
        self.save()?;
        Ok(sub)
    }

    pub fn set_subscription_active(&self, id: i64, is_active: bool) -> Result<(), String> {
        {
            let mut data = self.data.write();
            if let Some(sub) = data.subscriptions.iter_mut().find(|s| s.id == Some(id)) {
                sub.is_active = is_active;
            }
        }
        self.save()
    }

    pub fn delete_subscription(&self, id: i64) -> Result<(), String> {
        self.data.write().subscriptions.retain(|s| s.id != Some(id));
        self.save()
    }

    // ===== Templates =====

    pub fn templates(&self, server_id: i64) -> Vec<CommandTemplate> {
        self.data
            .read()
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .cloned()
            .collect()
    }

    pub fn template(&self, id: i64) -> Option<CommandTemplate> {
        self.data.read().templates.iter().find(|t| t.id == Some(id)).cloned()
    }

    pub fn create_template(&self, server_id: i64, draft: TemplateDraft) -> Result<i64, String> {
        let id = {
            let mut data = self.data.write();
            data.next_template_id += 1;
            let id = data.next_template_id;
            let ts = now();
            data.templates.push(CommandTemplate {
                id: Some(id),
                server_id,
                name: draft.name,
                topic: draft.topic,
                payload: draft.payload,
                payload_type: draft.payload_type.as_str().to_string(),
                qos: draft.qos,
                retain: draft.retain,
                description: draft.description,
                category: draft.category,
                use_count: 0,
                last_used_at: None,
                created_at: Some(ts.clone()),
                updated_at: Some(ts),
            });
            id
        };
        self.save()?;
        Ok(id)
    }

    pub fn update_template(&self, id: i64, draft: TemplateDraft) -> Result<(), String> {
        {
            let mut data = self.data.write();
            if let Some(t) = data.templates.iter_mut().find(|t| t.id == Some(id)) {
                t.name = draft.name;
                t.topic = draft.topic;
                t.payload = draft.payload;
                t.payload_type = draft.payload_type.as_str().to_string();
                t.qos = draft.qos;
                t.retain = draft.retain;
                t.description = draft.description;
                t.category = draft.category;
                t.updated_at = Some(now());
            }
        }
        self.save()
    }

    pub fn delete_template(&self, id: i64) -> Result<(), String> {
        self.data.write().templates.retain(|t| t.id != Some(id));
        self.save()
    }

    /// Bump the usage counter and return the up-to-date template.
    pub fn use_template(&self, id: i64) -> Result<Option<CommandTemplate>, String> {
        {
            let mut data = self.data.write();
            if let Some(t) = data.templates.iter_mut().find(|t| t.id == Some(id)) {
                t.use_count += 1;
                t.last_used_at = Some(now());
            }
        }
        self.save()?;
        Ok(self.template(id))
    }

    pub fn template_categories(&self, server_id: i64) -> Vec<String> {
        let mut categories: Vec<String> = self
            .data
            .read()
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .filter_map(|t| t.category.clone())
            .filter(|c| !c.trim().is_empty())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }

    pub fn export_templates(&self, server_id: i64) -> Result<String, String> {
        serde_json::to_string_pretty(&self.templates(server_id)).map_err(|e| e.to_string())
    }

    /// Import templates exported as JSON (same format as before). Returns the
    /// number of imported templates.
    pub fn import_templates(&self, server_id: i64, json: &str) -> Result<usize, String> {
        let templates: Vec<CommandTemplate> = serde_json::from_str(json).map_err(|e| format!("JSON解析错误: {e}"))?;
        let mut imported = 0;
        for t in templates {
            let draft = TemplateDraft {
                payload_type: t.format(),
                name: t.name,
                topic: t.topic,
                payload: t.payload,
                qos: t.qos,
                retain: t.retain,
                description: t.description,
                category: t.category,
            };
            if self.create_template(server_id, draft).is_ok() {
                imported += 1;
            }
        }
        Ok(imported)
    }

    pub fn duplicate_template(&self, id: i64, new_name: String) -> Result<i64, String> {
        let t = self.template(id).ok_or("模板不存在")?;
        let draft = TemplateDraft {
            payload_type: t.format(),
            name: new_name,
            topic: t.topic,
            payload: t.payload,
            qos: t.qos,
            retain: t.retain,
            description: t.description,
            category: t.category,
        };
        self.create_template(t.server_id, draft)
    }

    // ===== Scripts =====

    pub fn scripts(&self, server_id: i64) -> Vec<Script> {
        self.data
            .read()
            .scripts
            .iter()
            .filter(|s| s.server_id == server_id)
            .cloned()
            .collect()
    }

    pub fn enabled_scripts(&self, server_id: i64, script_type: &str) -> Vec<Script> {
        self.data
            .read()
            .scripts
            .iter()
            .filter(|s| s.server_id == server_id && s.enabled && s.script_type == script_type)
            .cloned()
            .collect()
    }

    pub fn create_script(
        &self,
        server_id: i64,
        name: String,
        script_type: String,
        code: String,
        enabled: bool,
        description: Option<String>,
    ) -> Result<i64, String> {
        let id = {
            let mut data = self.data.write();
            data.next_script_id += 1;
            let id = data.next_script_id;
            let ts = now();
            data.scripts.push(Script {
                id: Some(id),
                server_id,
                name,
                script_type,
                code,
                enabled,
                description,
                created_at: Some(ts.clone()),
                updated_at: Some(ts),
            });
            id
        };
        self.save()?;
        Ok(id)
    }

    pub fn update_script(
        &self,
        id: i64,
        name: String,
        code: String,
        description: Option<String>,
    ) -> Result<(), String> {
        {
            let mut data = self.data.write();
            if let Some(s) = data.scripts.iter_mut().find(|s| s.id == Some(id)) {
                s.name = name;
                s.code = code;
                s.description = description;
                s.updated_at = Some(now());
            }
        }
        self.save()
    }

    pub fn toggle_script(&self, id: i64, enabled: bool) -> Result<(), String> {
        {
            let mut data = self.data.write();
            if let Some(s) = data.scripts.iter_mut().find(|s| s.id == Some(id)) {
                s.enabled = enabled;
                s.updated_at = Some(now());
            }
        }
        self.save()
    }

    pub fn delete_script(&self, id: i64) -> Result<(), String> {
        self.data.write().scripts.retain(|s| s.id != Some(id));
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::models::PayloadFormat;

    fn temp_storage(name: &str) -> (Storage, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mini-mqtt-test-{name}-{}", uuid::Uuid::new_v4()));
        let path = dir.join("data.yaml");
        (Storage::open(path.clone()).unwrap(), dir)
    }

    #[test]
    fn crud_roundtrip_persists() {
        let (storage, dir) = temp_storage("crud");
        let id = storage
            .create_server(MqttServer {
                name: "a".into(),
                host: "localhost".into(),
                ..Default::default()
            })
            .unwrap();
        storage.create_subscription(id, "a/#".into(), 1).unwrap();
        assert!(storage.create_subscription(id, "a/#".into(), 1).is_err());
        let tid = storage
            .create_template(
                id,
                TemplateDraft {
                    name: "t".into(),
                    topic: "a/b".into(),
                    payload: "{}".into(),
                    payload_type: PayloadFormat::Json,
                    category: Some("c".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        storage.use_template(tid).unwrap();
        storage
            .create_script(id, "s".into(), "before_publish".into(), "1".into(), true, None)
            .unwrap();

        let reopened = Storage::open(storage.file_path().to_path_buf()).unwrap();
        assert_eq!(reopened.servers().len(), 1);
        assert_eq!(reopened.subscriptions(id).len(), 1);
        assert_eq!(reopened.template(tid).unwrap().use_count, 1);
        assert_eq!(reopened.template_categories(id), vec!["c".to_string()]);
        assert_eq!(reopened.enabled_scripts(id, "before_publish").len(), 1);

        let exported = reopened.export_templates(id).unwrap();
        assert_eq!(reopened.import_templates(id, &exported).unwrap(), 1);
        assert_eq!(reopened.templates(id).len(), 2);

        reopened.delete_server(id).unwrap();
        assert!(reopened.templates(id).is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
