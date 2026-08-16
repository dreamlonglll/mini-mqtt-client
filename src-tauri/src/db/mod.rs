pub mod models;

use models::{CommandTemplate, CreateTemplateRequest, CreateScriptRequest, MessageHistory, MqttServer, Script, Subscription, UpdateSubscriptionRequest, UpdateTemplateRequest, UpdateScriptRequest, EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;

/// 后台防抖落盘的检查间隔
const SAVE_DEBOUNCE: Duration = Duration::from_millis(1000);
/// 每个 server 内存中保留的最大消息条数
const MAX_MESSAGES_PER_SERVER: usize = 1000;

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct AppData {
    pub servers: Vec<MqttServer>,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
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
    next_template_id: i64,
    #[serde(default)]
    next_script_id: i64,
    #[serde(default)]
    next_env_variable_id: i64,
}

/// 应用配置（用于存储自定义数据路径等）
#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct AppConfig {
    pub data_path: Option<String>,
}

pub struct Storage {
    data: Arc<RwLock<AppData>>,
    /// 数据文件路径（迁移数据目录后可在运行中更新）
    file_path: Arc<RwLock<PathBuf>>,
    /// 脏标记：写操作只置位，由后台线程防抖落盘
    dirty: Arc<AtomicBool>,
    /// 串行化落盘：防止退出 flush 与后台线程并发写同一临时文件
    save_lock: Arc<parking_lot::Mutex<()>>,
    /// 消息历史仅保留在内存（按 server 分桶），不再写入 data.yaml
    messages: RwLock<HashMap<i64, VecDeque<MessageHistory>>>,
    next_message_id: AtomicI64,
}

impl Storage {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;

        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

        // 检查是否有自定义配置
        let config_path = app_dir.join("config.yaml");
        let file_path = if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = serde_yaml::from_str::<AppConfig>(&content) {
                    if let Some(custom_path) = config.data_path {
                        let custom_path = PathBuf::from(custom_path);
                        if custom_path.exists() || custom_path.parent().map(|p| p.exists()).unwrap_or(false) {
                            custom_path
                        } else {
                            app_dir.join("data.yaml")
                        }
                    } else {
                        app_dir.join("data.yaml")
                    }
                } else {
                    app_dir.join("data.yaml")
                }
            } else {
                app_dir.join("data.yaml")
            }
        } else {
            app_dir.join("data.yaml")
        };

        Self::open_at(file_path)
    }

    /// 按给定数据文件路径构造 Storage（`new` 解析出路径后调用；测试可直接传入临时目录路径）
    fn open_at(file_path: PathBuf) -> Result<Self, String> {
        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
            match serde_yaml::from_str(&content) {
                Ok(data) => data,
                Err(e) => {
                    // 解析失败：备份损坏文件后从空数据启动，避免静默覆盖用户数据
                    let backup = file_path.with_extension(format!(
                        "corrupt-{}.yaml",
                        chrono::Utc::now().format("%Y%m%d%H%M%S")
                    ));
                    let _ = fs::copy(&file_path, &backup);
                    eprintln!(
                        "Failed to parse {}: {}. Corrupt file backed up to {}",
                        file_path.display(),
                        e,
                        backup.display()
                    );
                    AppData::default()
                }
            }
        } else {
            AppData::default()
        };

        let data = Arc::new(RwLock::new(data));
        let dirty = Arc::new(AtomicBool::new(false));
        let file_path = Arc::new(RwLock::new(file_path));
        let save_lock = Arc::new(parking_lot::Mutex::new(()));

        // 后台防抖落盘线程：脏标记置位后最多 SAVE_DEBOUNCE 内落盘一次，
        // 避免每次写操作都全量序列化 + 写磁盘，也让 async 命令不再被同步 IO 阻塞
        {
            let data = Arc::clone(&data);
            let dirty = Arc::clone(&dirty);
            let file_path = Arc::clone(&file_path);
            let save_lock = Arc::clone(&save_lock);
            std::thread::spawn(move || loop {
                std::thread::sleep(SAVE_DEBOUNCE);
                if dirty.swap(false, Ordering::AcqRel) {
                    let _guard = save_lock.lock();
                    if let Err(e) = Self::save_to_disk(&data, &file_path) {
                        eprintln!("Failed to save data: {}", e);
                        // 落盘失败时重新置脏，下个周期重试
                        dirty.store(true, Ordering::Release);
                    }
                }
            });
        }

        Ok(Self {
            data,
            file_path,
            dirty,
            save_lock,
            messages: RwLock::new(HashMap::new()),
            next_message_id: AtomicI64::new(0),
        })
    }

    /// 获取当前数据文件路径
    pub fn get_file_path(&self) -> PathBuf {
        self.file_path.read().clone()
    }

    /// 更新数据文件路径（数据目录迁移后调用，后续落盘写入新位置）
    pub fn set_file_path(&self, path: PathBuf) {
        *self.file_path.write() = path;
    }

    /// 标记数据已修改，等待后台线程防抖落盘
    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    /// 立即将内存数据写入磁盘（应用退出前调用）
    ///
    /// 这里**不检查脏标记**：后台防抖线程是先 `dirty.swap(false)` 再写盘，
    /// 若恰好在 swap 之后、写完之前退出，按脏标记判断会跳过落盘导致最后一次修改丢失。
    /// 退出场景多写一次的成本可忽略，故无条件落盘。
    pub fn flush(&self) {
        let _guard = self.save_lock.lock();
        if let Err(e) = Self::save_to_disk(&self.data, &self.file_path) {
            eprintln!("Failed to flush data on exit: {}", e);
        }
    }

    fn save_to_disk(data: &RwLock<AppData>, file_path: &RwLock<PathBuf>) -> Result<(), String> {
        let content = {
            let data = data.read();
            serde_yaml::to_string(&*data).map_err(|e| e.to_string())?
        };
        let path = file_path.read().clone();
        Self::write_atomic(&path, &content)
    }

    /// 写临时文件 + rename 原子替换，避免崩溃/断电留下半截文件
    fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
        let tmp = path.with_extension("yaml.tmp");
        fs::write(&tmp, content).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    // ===== Server 操作 =====
    /// 在锁 guard 上直接序列化返回，避免连同证书等大字段一起深拷贝
    pub fn get_servers_json(&self) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        serde_json::to_value(&data.servers).map_err(|e| e.to_string())
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
        self.mark_dirty();
        Ok(id)
    }

    pub fn update_server(&self, server: MqttServer) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(existing) = data.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
            existing.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_server(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.servers.retain(|s| s.id != Some(id));
        // 同时删除相关订阅、模板、脚本和环境变量
        data.subscriptions.retain(|s| s.server_id != id);
        data.templates.retain(|t| t.server_id != id);
        data.scripts.retain(|s| s.server_id != id);
        data.env_variables.retain(|e| e.server_id != id);
        drop(data);
        // 内存中的消息历史一并清理
        self.messages.write().remove(&id);
        self.mark_dirty();
        Ok(())
    }

    // ===== 订阅操作 =====
    pub fn get_subscriptions_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&Subscription> = data
            .subscriptions
            .iter()
            .filter(|s| s.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn create_subscription(&self, mut sub: Subscription) -> Result<Subscription, String> {
        let mut data = self.data.write();
        data.next_subscription_id += 1;
        sub.id = Some(data.next_subscription_id);
        sub.created_at = Some(chrono::Utc::now().to_rfc3339());
        let result = sub.clone();
        data.subscriptions.push(sub);
        drop(data);
        self.mark_dirty();
        Ok(result)
    }

    pub fn update_subscription_status(&self, id: i64, is_active: bool) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(sub) = data.subscriptions.iter_mut().find(|s| s.id == Some(id)) {
            sub.is_active = is_active;
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn update_subscription(&self, req: UpdateSubscriptionRequest) -> Result<Subscription, String> {
        let mut data = self.data.write();
        if let Some(sub) = data.subscriptions.iter_mut().find(|s| s.id == Some(req.id)) {
            if let Some(topic) = req.topic {
                sub.topic = topic;
            }
            if let Some(qos) = req.qos {
                sub.qos = qos;
            }
            // color 可以设置为 None（清除颜色）
            sub.color = req.color;
            let result = sub.clone();
            drop(data);
            self.mark_dirty();
            Ok(result)
        } else {
            Err("Subscription not found".to_string())
        }
    }

    pub fn delete_subscription(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.subscriptions.retain(|s| s.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    // ===== 消息操作（仅内存，不落盘） =====
    pub fn get_messages(&self, server_id: i64, limit: usize, offset: usize) -> Vec<MessageHistory> {
        let messages = self.messages.read();
        messages
            .get(&server_id)
            .map(|queue| queue.iter().rev().skip(offset).take(limit).cloned().collect())
            .unwrap_or_default()
    }

    pub fn create_message(&self, mut msg: MessageHistory) -> Result<MessageHistory, String> {
        msg.id = Some(self.next_message_id.fetch_add(1, Ordering::Relaxed) + 1);
        msg.created_at = Some(chrono::Utc::now().to_rfc3339());
        let result = msg.clone();

        let mut messages = self.messages.write();
        let queue = messages.entry(result.server_id).or_default();
        queue.push_back(msg);
        if queue.len() > MAX_MESSAGES_PER_SERVER {
            queue.pop_front();
        }
        Ok(result)
    }

    pub fn clear_messages(&self, server_id: i64) -> Result<(), String> {
        self.messages.write().remove(&server_id);
        Ok(())
    }

    // ===== 模板操作 =====
    pub fn get_templates_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&CommandTemplate> = data
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
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
        self.mark_dirty();
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
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_template(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.templates.retain(|t| t.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn increment_template_use_count(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(template) = data.templates.iter_mut().find(|t| t.id == Some(id)) {
            template.use_count += 1;
            template.last_used_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
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

    // ===== 脚本操作 =====
    pub fn get_scripts_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&Script> = data
            .scripts
            .iter()
            .filter(|s| s.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn get_script(&self, id: i64) -> Option<Script> {
        let data = self.data.read();
        data.scripts.iter().find(|s| s.id == Some(id)).cloned()
    }

    pub fn get_enabled_scripts_json(
        &self,
        server_id: i64,
        script_type: &str,
    ) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&Script> = data
            .scripts
            .iter()
            .filter(|s| s.server_id == server_id && s.enabled && s.script_type == script_type)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn create_script(&self, req: CreateScriptRequest) -> Result<i64, String> {
        let mut data = self.data.write();
        data.next_script_id += 1;
        let id = data.next_script_id;
        let now = chrono::Utc::now().to_rfc3339();
        
        let script = Script {
            id: Some(id),
            server_id: req.server_id,
            name: req.name,
            script_type: req.script_type,
            code: req.code,
            enabled: req.enabled,
            description: req.description,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };
        
        data.scripts.push(script);
        drop(data);
        self.mark_dirty();
        Ok(id)
    }

    pub fn update_script(&self, req: UpdateScriptRequest) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(script) = data.scripts.iter_mut().find(|s| s.id == Some(req.id)) {
            if let Some(name) = req.name {
                script.name = name;
            }
            if let Some(code) = req.code {
                script.code = code;
            }
            if let Some(enabled) = req.enabled {
                script.enabled = enabled;
            }
            if let Some(description) = req.description {
                script.description = Some(description);
            }
            script.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_script(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.scripts.retain(|s| s.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn toggle_script(&self, id: i64, enabled: bool) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(script) = data.scripts.iter_mut().find(|s| s.id == Some(id)) {
            script.enabled = enabled;
            script.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    // ===== 环境变量操作 =====
    pub fn get_env_variables_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&EnvVariable> = data
            .env_variables
            .iter()
            .filter(|e| e.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn get_env_variable(&self, id: i64) -> Option<EnvVariable> {
        let data = self.data.read();
        data.env_variables.iter().find(|e| e.id == Some(id)).cloned()
    }

    pub fn create_env_variable(&self, req: CreateEnvVariableRequest) -> Result<i64, String> {
        let mut data = self.data.write();
        
        // 检查变量名是否重复
        let exists = data.env_variables.iter().any(|e| {
            e.server_id == req.server_id && e.name == req.name
        });
        if exists {
            return Err("Variable name already exists".to_string());
        }

        data.next_env_variable_id += 1;
        let id = data.next_env_variable_id;
        let now = chrono::Utc::now().to_rfc3339();
        
        let env_var = EnvVariable {
            id: Some(id),
            server_id: req.server_id,
            name: req.name,
            value: req.value,
            description: req.description,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };
        
        data.env_variables.push(env_var);
        drop(data);
        self.mark_dirty();
        Ok(id)
    }

    pub fn update_env_variable(&self, req: UpdateEnvVariableRequest) -> Result<(), String> {
        let mut data = self.data.write();
        
        // 如果要更新名称，检查是否与其他变量重复
        if let Some(new_name) = &req.name {
            let current = data.env_variables.iter().find(|e| e.id == Some(req.id));
            if let Some(current) = current {
                let exists = data.env_variables.iter().any(|e| {
                    e.server_id == current.server_id && e.name == *new_name && e.id != Some(req.id)
                });
                if exists {
                    return Err("Variable name already exists".to_string());
                }
            }
        }

        if let Some(env_var) = data.env_variables.iter_mut().find(|e| e.id == Some(req.id)) {
            if let Some(name) = req.name {
                env_var.name = name;
            }
            if let Some(value) = req.value {
                env_var.value = value;
            }
            if let Some(description) = req.description {
                env_var.description = Some(description);
            }
            env_var.updated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_env_variable(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.env_variables.retain(|e| e.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 在临时目录里准备一个数据文件路径（TempDir 需由调用方持有，drop 后目录被清理）
    fn temp_data_path() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("创建临时目录失败");
        let path = dir.path().join("data.yaml");
        (dir, path)
    }

    fn sample_server(name: &str) -> MqttServer {
        MqttServer {
            id: None,
            name: name.to_string(),
            host: "127.0.0.1".to_string(),
            port: 1883,
            protocol_version: "3.1.1".to_string(),
            username: None,
            password: None,
            client_id: Some("test-client".to_string()),
            keep_alive: 60,
            clean_session: true,
            use_tls: false,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            client_key_password: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn sample_template(server_id: i64, name: &str) -> CreateTemplateRequest {
        CreateTemplateRequest {
            server_id,
            name: name.to_string(),
            topic: "test/topic".to_string(),
            payload: "{\"a\":1}".to_string(),
            payload_type: "json".to_string(),
            qos: 1,
            retain: false,
            description: Some("测试模板".to_string()),
            category: Some("默认".to_string()),
        }
    }

    fn server_count(storage: &Storage) -> usize {
        storage
            .get_servers_json()
            .unwrap()
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    }

    /// 列出数据目录下的损坏备份文件名
    fn corrupt_backups(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("data.corrupt-"))
            .collect()
    }

    #[test]
    fn server_crud_survives_flush_and_reload() {
        let (_dir, path) = temp_data_path();
        let storage = Storage::open_at(path.clone()).unwrap();

        let keep_id = storage.create_server(sample_server("保留的服务器")).unwrap();
        let drop_id = storage.create_server(sample_server("待删除的服务器")).unwrap();

        let mut updated = storage.get_server(keep_id).unwrap();
        updated.name = "保留的服务器-已改名".to_string();
        updated.port = 8883;
        storage.update_server(updated).unwrap();
        storage.delete_server(drop_id).unwrap();

        storage.flush();
        drop(storage);

        let reloaded = Storage::open_at(path).unwrap();
        assert_eq!(server_count(&reloaded), 1);
        let server = reloaded.get_server(keep_id).expect("保留的服务器应被读回");
        assert_eq!(server.name, "保留的服务器-已改名");
        assert_eq!(server.port, 8883);
        assert!(server.created_at.is_some());
        assert!(reloaded.get_server(drop_id).is_none());
    }

    #[test]
    fn template_crud_survives_flush_and_reload() {
        let (_dir, path) = temp_data_path();
        let storage = Storage::open_at(path.clone()).unwrap();

        let template_id = storage.create_template(sample_template(1, "模板A")).unwrap();
        storage
            .create_template(sample_template(2, "另一个 server 的模板"))
            .unwrap();

        storage
            .update_template(UpdateTemplateRequest {
                id: template_id,
                name: Some("模板A-已改名".to_string()),
                topic: None,
                payload: Some("DEADBEEF".to_string()),
                payload_type: Some("hex".to_string()),
                qos: Some(2),
                retain: Some(true),
                description: None,
                category: None,
            })
            .unwrap();
        storage.increment_template_use_count(template_id).unwrap();

        storage.flush();
        drop(storage);

        let reloaded = Storage::open_at(path).unwrap();
        let template = reloaded.get_template(template_id).expect("模板应被读回");
        assert_eq!(template.name, "模板A-已改名");
        assert_eq!(template.topic, "test/topic");
        assert_eq!(template.payload, "DEADBEEF");
        assert_eq!(template.payload_type, "hex");
        assert_eq!(template.qos, 2);
        assert!(template.retain);
        assert_eq!(template.use_count, 1);
        assert!(template.last_used_at.is_some());

        // 按 server 过滤的读取在重载后仍然正确
        let server1 = reloaded.get_templates_json(1).unwrap();
        assert_eq!(server1.as_array().unwrap().len(), 1);
        let server2 = reloaded.get_templates_json(2).unwrap();
        assert_eq!(server2.as_array().unwrap().len(), 1);
        assert_eq!(reloaded.get_template_categories(1), vec!["默认".to_string()]);
    }

    #[test]
    fn ids_keep_increasing_after_reload() {
        let (_dir, path) = temp_data_path();
        let storage = Storage::open_at(path.clone()).unwrap();
        let first_id = storage.create_server(sample_server("第一个")).unwrap();
        storage.delete_server(first_id).unwrap();
        storage.flush();
        drop(storage);

        let reloaded = Storage::open_at(path).unwrap();
        let second_id = reloaded.create_server(sample_server("第二个")).unwrap();
        assert!(
            second_id > first_id,
            "重载后新建的 ID 不应与已用过的 ID 冲突：{} vs {}",
            second_id,
            first_id
        );
    }

    /// flush() 必须无条件落盘：脏标记已被后台线程取走（swap 为 false）时，
    /// 退出前的 flush 仍要把内存数据写到磁盘，否则最后一次修改丢失。
    #[test]
    fn flush_writes_even_when_dirty_flag_already_consumed() {
        let (_dir, path) = temp_data_path();
        let storage = Storage::open_at(path.clone()).unwrap();
        storage.create_server(sample_server("退出前的最后一次修改")).unwrap();

        // 第一次 flush 后脏标记已被消费，等价于"后台线程刚 swap 完"的状态
        storage.flush();
        assert!(path.exists());

        // 删掉文件模拟"脏标记已取走但写盘尚未完成"，此时退出前的 flush 必须重新落盘
        fs::remove_file(&path).unwrap();
        storage.flush();
        assert!(path.exists(), "flush() 检查脏标记会跳过落盘，导致最后一次修改丢失");

        let reloaded = Storage::open_at(path).unwrap();
        assert_eq!(server_count(&reloaded), 1);
        assert_eq!(
            reloaded.get_server(1).unwrap().name,
            "退出前的最后一次修改"
        );
    }

    #[test]
    fn corrupt_data_file_is_backed_up_and_storage_starts_empty() {
        let (dir, path) = temp_data_path();
        let corrupt = "servers: [unclosed\nnext_server_id: \"not-a-number\"\n";
        fs::write(&path, corrupt).unwrap();

        let storage = Storage::open_at(path.clone()).unwrap();
        assert_eq!(server_count(&storage), 0, "损坏文件应以空数据启动");

        let backups = corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1, "损坏文件应被备份为 data.corrupt-*，实际：{:?}", backups);
        let backup_content = fs::read_to_string(dir.path().join(&backups[0])).unwrap();
        assert_eq!(backup_content, corrupt, "备份内容应与损坏原文一致");

        // 损坏恢复后仍可正常写入并落盘
        storage.create_server(sample_server("恢复后新建")).unwrap();
        storage.flush();
        let reloaded = Storage::open_at(path).unwrap();
        assert_eq!(server_count(&reloaded), 1);
    }
}
