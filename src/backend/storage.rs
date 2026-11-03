//! YAML-file storage, ported from the Tauri backend (`src-tauri/src/db`).

use super::logger::{LogKind, LogManager};
use super::models::{
    CommandTemplate, EnvVariable, Extra, MessageHistory, MqttServer, Script, Subscription, TemplateDraft,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    pub env_variables: Vec<EnvVariable>,
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
    #[serde(default)]
    next_env_variable_id: i64,
    /// Top-level keys this version does not model, written back unchanged.
    #[serde(flatten)]
    extra: Extra,
}

impl AppData {
    /// Guard against hand-edited files whose counters lag behind the data:
    /// a reused id would make `delete` (which retains by id) remove two items.
    fn fix_counters(&mut self) {
        let max = |ids: &mut dyn Iterator<Item = Option<i64>>| ids.flatten().max().unwrap_or(0);
        self.next_server_id = self.next_server_id.max(max(&mut self.servers.iter().map(|s| s.id)));
        self.next_subscription_id = self
            .next_subscription_id
            .max(max(&mut self.subscriptions.iter().map(|s| s.id)));
        self.next_message_id = self.next_message_id.max(max(&mut self.messages.iter().map(|s| s.id)));
        self.next_template_id = self.next_template_id.max(max(&mut self.templates.iter().map(|s| s.id)));
        self.next_script_id = self.next_script_id.max(max(&mut self.scripts.iter().map(|s| s.id)));
        self.next_env_variable_id = self
            .next_env_variable_id
            .max(max(&mut self.env_variables.iter().map(|e| e.id)));
    }
}

/// Write `content` to `path` atomically: a temporary file that is flushed to
/// disk before it replaces the target, so a crash or power loss leaves either
/// the old or the new file, never a truncated one.
pub fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(content)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

pub struct Storage {
    data: RwLock<AppData>,
    /// Current data file; switched in place when the data location is migrated.
    file_path: RwLock<PathBuf>,
    /// Where an unreadable data file was copied to when it was opened.
    corrupt_backup: Option<PathBuf>,
    logger: Option<Arc<LogManager>>,
}

impl Storage {
    pub fn open(file_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut corrupt_backup = None;
        let mut data: AppData = if file_path.exists() {
            let bytes = fs::read(&file_path).map_err(|e| e.to_string())?;
            let parsed = std::str::from_utf8(&bytes)
                .map_err(|e| e.to_string())
                .and_then(|text| serde_yaml::from_str(text).map_err(|e| e.to_string()));
            match parsed {
                Ok(data) => data,
                Err(err) => {
                    // Never silently overwrite a file we could not parse: keep a
                    // timestamped copy (an earlier backup is not overwritten).
                    let backup = file_path
                        .with_extension(format!("corrupt-{}.yaml", chrono::Local::now().format("%Y%m%d%H%M%S")));
                    fs::copy(&file_path, &backup).map_err(|e| format!("数据文件无法解析，且备份失败: {e}（{err}）"))?;
                    log::error!("data file is invalid ({err}); backed up to {}", backup.display());
                    corrupt_backup = Some(backup);
                    AppData::default()
                }
            }
        } else {
            AppData::default()
        };
        data.fix_counters();
        Ok(Self {
            data: RwLock::new(data),
            file_path: RwLock::new(file_path),
            corrupt_backup,
            logger: None,
        })
    }

    /// Write failures to the error log as well (they are also returned).
    pub fn with_logger(mut self, logger: Arc<LogManager>) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn file_path(&self) -> PathBuf {
        self.file_path.read().clone()
    }

    /// Continue writing to `path` (after the data was migrated there).
    pub fn set_file_path(&self, path: PathBuf) {
        *self.file_path.write() = path;
    }

    pub fn corrupt_backup(&self) -> Option<&Path> {
        self.corrupt_backup.as_deref()
    }

    /// Write everything to disk. A failed write leaves the change in memory,
    /// so the next successful save persists it.
    pub fn save(&self) -> Result<(), String> {
        let content = {
            let data = self.data.read();
            serde_yaml::to_string(&*data).map_err(|e| e.to_string())?
        };
        let path = self.file_path();
        write_atomic(&path, content.as_bytes()).map_err(|e| {
            let message = format!("保存数据失败: {e}");
            if let Some(logger) = &self.logger {
                logger.write(LogKind::Storage, &message, Some(&path.display().to_string()));
            }
            message
        })
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
            let existing = data
                .servers
                .iter_mut()
                .find(|s| s.id.is_some() && s.id == server.id)
                .ok_or("服务器不存在")?;
            let created_at = existing.created_at.clone();
            *existing = server;
            existing.created_at = created_at;
            existing.updated_at = Some(now());
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
            data.env_variables.retain(|e| e.server_id != id);
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

    pub fn create_subscription(
        &self,
        server_id: i64,
        topic: String,
        qos: i32,
        color: Option<String>,
    ) -> Result<Subscription, String> {
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
                color,
                created_at: Some(now()),
                extra: Extra::new(),
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
            let sub = data
                .subscriptions
                .iter_mut()
                .find(|s| s.id == Some(id))
                .ok_or("订阅不存在")?;
            sub.is_active = is_active;
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
                extra: Extra::new(),
            });
            id
        };
        self.save()?;
        Ok(id)
    }

    pub fn update_template(&self, id: i64, draft: TemplateDraft) -> Result<(), String> {
        {
            let mut data = self.data.write();
            let t = data
                .templates
                .iter_mut()
                .find(|t| t.id == Some(id))
                .ok_or("模板不存在")?;
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
        self.save()
    }

    pub fn delete_template(&self, id: i64) -> Result<(), String> {
        self.data.write().templates.retain(|t| t.id != Some(id));
        self.save()
    }

    /// Bump the usage counter and return the up-to-date template.
    pub fn use_template(&self, id: i64) -> Result<CommandTemplate, String> {
        let template = {
            let mut data = self.data.write();
            let t = data
                .templates
                .iter_mut()
                .find(|t| t.id == Some(id))
                .ok_or("模板不存在")?;
            t.use_count += 1;
            t.last_used_at = Some(now());
            t.clone()
        };
        self.save()?;
        Ok(template)
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
                extra: Extra::new(),
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
            let s = data.scripts.iter_mut().find(|s| s.id == Some(id)).ok_or("脚本不存在")?;
            s.name = name;
            s.code = code;
            s.description = description;
            s.updated_at = Some(now());
        }
        self.save()
    }

    pub fn toggle_script(&self, id: i64, enabled: bool) -> Result<(), String> {
        {
            let mut data = self.data.write();
            let s = data.scripts.iter_mut().find(|s| s.id == Some(id)).ok_or("脚本不存在")?;
            s.enabled = enabled;
            s.updated_at = Some(now());
        }
        self.save()
    }

    pub fn delete_script(&self, id: i64) -> Result<(), String> {
        self.data.write().scripts.retain(|s| s.id != Some(id));
        self.save()
    }

    // ===== Environment variables =====

    pub fn env_variables(&self, server_id: i64) -> Vec<EnvVariable> {
        self.data
            .read()
            .env_variables
            .iter()
            .filter(|e| e.server_id == server_id)
            .cloned()
            .collect()
    }

    /// `(name, value)` pairs of a server, for `{{NAME}}` substitution.
    pub fn env_pairs(&self, server_id: i64) -> Vec<(String, String)> {
        self.data
            .read()
            .env_variables
            .iter()
            .filter(|e| e.server_id == server_id)
            .map(|e| (e.name.clone(), e.value.clone()))
            .collect()
    }

    pub fn create_env_variable(
        &self,
        server_id: i64,
        name: String,
        value: String,
        description: Option<String>,
    ) -> Result<i64, String> {
        let id = {
            let mut data = self.data.write();
            if data
                .env_variables
                .iter()
                .any(|e| e.server_id == server_id && e.name == name)
            {
                return Err("变量名已存在".into());
            }
            data.next_env_variable_id += 1;
            let id = data.next_env_variable_id;
            let ts = now();
            data.env_variables.push(EnvVariable {
                id: Some(id),
                server_id,
                name,
                value,
                description,
                created_at: Some(ts.clone()),
                updated_at: Some(ts),
                extra: Extra::new(),
            });
            id
        };
        self.save()?;
        Ok(id)
    }

    pub fn update_env_variable(
        &self,
        id: i64,
        name: String,
        value: String,
        description: Option<String>,
    ) -> Result<(), String> {
        {
            let mut data = self.data.write();
            let server_id = data
                .env_variables
                .iter()
                .find(|e| e.id == Some(id))
                .map(|e| e.server_id)
                .ok_or("环境变量不存在")?;
            if data
                .env_variables
                .iter()
                .any(|e| e.server_id == server_id && e.name == name && e.id != Some(id))
            {
                return Err("变量名已存在".into());
            }
            if let Some(e) = data.env_variables.iter_mut().find(|e| e.id == Some(id)) {
                e.name = name;
                e.value = value;
                e.description = description;
                e.updated_at = Some(now());
            }
        }
        self.save()
    }

    pub fn delete_env_variable(&self, id: i64) -> Result<(), String> {
        self.data.write().env_variables.retain(|e| e.id != Some(id));
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::models::PayloadFormat;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mini-mqtt-test-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn temp_storage(name: &str) -> (Storage, PathBuf) {
        let dir = temp_dir(name);
        let path = dir.join("data.yaml");
        (Storage::open(path).unwrap(), dir)
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
        storage.create_subscription(id, "a/#".into(), 1, None).unwrap();
        assert!(storage.create_subscription(id, "a/#".into(), 1, None).is_err());
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

        let reopened = Storage::open(storage.file_path()).unwrap();
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

    #[test]
    fn env_variables_are_per_server_and_unique_by_name() {
        let (storage, dir) = temp_storage("env");
        let a = storage.create_server(MqttServer::default()).unwrap();
        let b = storage.create_server(MqttServer::default()).unwrap();
        let id = storage
            .create_env_variable(a, "HOST".into(), "a.local".into(), Some("desc".into()))
            .unwrap();
        assert!(storage.create_env_variable(a, "HOST".into(), "x".into(), None).is_err());
        storage
            .create_env_variable(b, "HOST".into(), "b.local".into(), None)
            .unwrap();
        let other = storage.create_env_variable(a, "PORT".into(), "1".into(), None).unwrap();
        assert_eq!(
            storage.env_pairs(a),
            vec![("HOST".into(), "a.local".into()), ("PORT".into(), "1".into())]
        );
        assert_eq!(storage.env_pairs(b), vec![("HOST".into(), "b.local".into())]);

        assert!(
            storage
                .update_env_variable(other, "HOST".into(), "1".into(), None)
                .is_err(),
            "rename onto an existing name"
        );
        storage
            .update_env_variable(id, "HOST".into(), "new".into(), None)
            .unwrap();
        assert!(storage.update_env_variable(999, "X".into(), "1".into(), None).is_err());
        let reopened = Storage::open(storage.file_path()).unwrap();
        let vars = reopened.env_variables(a);
        assert_eq!(vars[0].value, "new");
        assert_eq!(vars[0].description, None, "description can be cleared");

        reopened.delete_env_variable(other).unwrap();
        assert_eq!(reopened.env_variables(a).len(), 1);
        reopened.delete_server(a).unwrap();
        assert!(
            reopened.env_variables(a).is_empty(),
            "deleting a server deletes its variables"
        );
        assert_eq!(reopened.env_variables(b).len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn updates_of_missing_items_fail_without_writing() {
        let (storage, dir) = temp_storage("missing");
        let missing = MqttServer {
            id: Some(42),
            ..Default::default()
        };
        assert!(storage.update_server(missing).is_err());
        assert!(storage.set_subscription_active(7, false).is_err());
        assert!(storage.update_template(7, TemplateDraft::default()).is_err());
        assert!(storage.use_template(7).is_err());
        assert!(storage.update_script(7, "n".into(), "c".into(), None).is_err());
        assert!(storage.toggle_script(7, true).is_err());
        assert!(!storage.file_path().exists(), "nothing changed, nothing written");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unreadable_file_is_backed_up_not_overwritten() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.yaml");
        fs::write(&path, b"servers: [\xff\xfe not yaml").unwrap();
        let storage = Storage::open(path.clone()).unwrap();
        let backup = storage.corrupt_backup().expect("backup created").to_path_buf();
        assert_eq!(fs::read(&backup).unwrap(), b"servers: [\xff\xfe not yaml");
        assert!(storage.servers().is_empty());
        storage.create_server(MqttServer::default()).unwrap();
        assert!(backup.exists(), "saving keeps the backup");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn atomic_write_replaces_file_and_leaves_no_temp() {
        let dir = temp_dir("atomic");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.yaml");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// A `data.yaml` as written by v1.7.1, plus keys no version writes yet.
    const LEGACY_V171: &str = r#"
servers:
- id: 1
  name: 生产环境
  host: broker.local
  port: 8883
  protocol_version: '5.0'
  username: user
  password: secret
  client_id: ''
  keep_alive: 60
  clean_session: true
  use_tls: true
  ca_cert: null
  client_cert: |
    -----BEGIN CERTIFICATE-----
    MIIB
    -----END CERTIFICATE-----
  client_key: |
    -----BEGIN ENCRYPTED PRIVATE KEY-----
    MIIC
    -----END ENCRYPTED PRIVATE KEY-----
  client_key_password: key-pass
  created_at: 2025-01-01T00:00:00Z
  updated_at: 2025-01-02T00:00:00Z
  future_server_field: kept
subscriptions:
- id: 3
  server_id: 1
  topic: device/{{DEVICE_ID}}/#
  qos: 1
  is_active: true
  color: '#F56C6C'
  created_at: 2025-01-01T00:00:00Z
- id: 4
  server_id: 1
  topic: sensor/+/temp
  qos: 0
  is_active: false
  color: rgba(64, 158, 255, 0.5)
  created_at: 2025-01-01T00:00:00Z
templates:
- id: 2
  server_id: 1
  name: 开灯
  topic: device/{{DEVICE_ID}}/cmd
  payload: '{"token":"{{API_KEY}}"}'
  payload_type: json
  qos: 1
  retain: false
  description: null
  category: 照明
  use_count: 5
  last_used_at: null
  created_at: 2025-01-01T00:00:00Z
  updated_at: 2025-01-01T00:00:00Z
scripts:
- id: 5
  server_id: 1
  name: 签名
  script_type: before_publish
  code: function process(p) { return env.replace(p); }
  enabled: true
  description: null
  created_at: 2025-01-01T00:00:00Z
  updated_at: 2025-01-01T00:00:00Z
env_variables:
- id: 6
  server_id: 1
  name: DEVICE_ID
  value: dev-001
  description: 设备编号
  created_at: 2025-01-01T00:00:00Z
  updated_at: 2025-01-01T00:00:00Z
- id: 7
  server_id: 1
  name: API_KEY
  value: k-123
  description: null
  created_at: 2025-01-01T00:00:00Z
  updated_at: 2025-01-01T00:00:00Z
next_server_id: 1
next_subscription_id: 4
next_template_id: 2
next_script_id: 5
next_env_variable_id: 7
future_top_level:
  nested: [1, 2]
"#;

    /// Every key/value of `old` is present, unchanged, in `new`.
    fn assert_preserved(old: &serde_yaml::Value, new: &serde_yaml::Value, path: &str) {
        use serde_yaml::Value;
        match (old, new) {
            (Value::Mapping(o), Value::Mapping(n)) => {
                for (key, value) in o {
                    let key_name = key.as_str().unwrap_or("?");
                    let child = n.get(key).unwrap_or_else(|| panic!("{path}.{key_name} was dropped"));
                    assert_preserved(value, child, &format!("{path}.{key_name}"));
                }
            }
            (Value::Sequence(o), Value::Sequence(n)) => {
                assert_eq!(o.len(), n.len(), "{path}: length changed");
                for (ix, (a, b)) in o.iter().zip(n).enumerate() {
                    assert_preserved(a, b, &format!("{path}[{ix}]"));
                }
            }
            _ => assert_eq!(old, new, "{path} changed"),
        }
    }

    #[test]
    fn legacy_v171_data_survives_load_save_load() {
        let dir = temp_dir("legacy");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.yaml");
        fs::write(&path, LEGACY_V171).unwrap();

        let storage = Storage::open(path.clone()).unwrap();
        let server = &storage.servers()[0];
        assert_eq!(server.client_key_password.as_deref(), Some("key-pass"));
        let subs = storage.subscriptions(1);
        assert_eq!(subs[0].color.as_deref(), Some("#F56C6C"));
        assert_eq!(subs[1].color.as_deref(), Some("rgba(64, 158, 255, 0.5)"));
        assert_eq!(storage.data.read().env_variables.len(), 2);

        // Any write rewrites the whole file: nothing may get lost.
        storage.set_subscription_active(4, true).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        let mut old: serde_yaml::Value = serde_yaml::from_str(LEGACY_V171).unwrap();
        old["subscriptions"][1]["is_active"] = serde_yaml::Value::from(true);
        let new: serde_yaml::Value = serde_yaml::from_str(&saved).unwrap();
        assert_preserved(&old, &new, "data");

        // ...and the saved file loads again with the same content.
        let reopened = Storage::open(path.clone()).unwrap();
        assert_eq!(reopened.servers(), storage.servers());
        assert_eq!(reopened.subscriptions(1), storage.subscriptions(1));
        assert_eq!(reopened.data.read().env_variables, storage.data.read().env_variables);
        reopened.update_server(reopened.servers()[0].clone()).unwrap();
        let resaved: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(resaved["future_top_level"], new["future_top_level"]);
        assert_eq!(
            resaved["servers"][0]["future_server_field"],
            serde_yaml::Value::from("kept")
        );

        // Counters continue after the legacy ids.
        let id = reopened.create_subscription(1, "new/topic".into(), 0, None).unwrap().id;
        assert_eq!(id, Some(5));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn counters_are_raised_to_existing_ids() {
        let dir = temp_dir("counters");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.yaml");
        // Hand-edited file without counters.
        fs::write(
            &path,
            "env_variables:\n- {id: 9, server_id: 1, name: A, value: x}\nservers:\n- {id: 4, name: s, host: h}\n",
        )
        .unwrap();
        let storage = Storage::open(path).unwrap();
        assert_eq!(storage.data.read().next_env_variable_id, 9);
        assert_eq!(storage.create_server(MqttServer::default()).unwrap(), 5);
        let _ = fs::remove_dir_all(dir);
    }
}
