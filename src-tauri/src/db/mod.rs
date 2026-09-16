pub mod models;

// 各实体的读写操作按实体拆分到子模块，均以 `impl Storage` 的形式挂在同一个类型上；
// 本文件只保留数据结构、加载/落盘与 ID 计数器等所有实体共用的核心。
mod env;
mod script;
mod server;
mod subscription;
mod template;

#[cfg(test)]
mod tests;

use models::{CommandTemplate, EnvVariable, MqttServer, Script, Subscription};
use parking_lot::{Condvar, Mutex, RwLock};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;

/// 脏标记置位后到真正落盘的合并窗口：连续写操作只序列化一次
const SAVE_DEBOUNCE: Duration = Duration::from_millis(1000);

/// 持久化失败的通知回调
///
/// 生产环境由 `Storage::new` 注入"向前端 emit `storage-error` + 写日志"的实现；
/// 测试可注入普通闭包，从而无需 AppHandle 即可覆盖失败通知路径。
pub type StorageErrorNotifier = Arc<dyn Fn(String) + Send + Sync>;

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

/// 脏标记 + 条件变量
///
/// 写操作置位并唤醒落盘线程；线程在没有写入时阻塞在条件变量上，
/// 不再每秒醒来轮询一次（对空闲时的 CPU 与笔记本续航友好）。
struct DirtySignal {
    dirty: Mutex<bool>,
    wakeup: Condvar,
}

impl DirtySignal {
    fn new() -> Self {
        Self {
            dirty: Mutex::new(false),
            wakeup: Condvar::new(),
        }
    }

    /// 置脏并唤醒落盘线程
    fn mark(&self) {
        *self.dirty.lock() = true;
        self.wakeup.notify_one();
    }

    /// 阻塞直到脏标记被置位（不消费标记）
    fn wait_until_dirty(&self) {
        let mut dirty = self.dirty.lock();
        while !*dirty {
            self.wakeup.wait(&mut dirty);
        }
    }

    /// 取走脏标记（返回取走前的值）
    fn take(&self) -> bool {
        std::mem::replace(&mut *self.dirty.lock(), false)
    }

    #[cfg(test)]
    fn is_dirty(&self) -> bool {
        *self.dirty.lock()
    }

    #[cfg(test)]
    fn clear(&self) {
        *self.dirty.lock() = false;
    }
}

pub struct Storage {
    data: Arc<RwLock<AppData>>,
    /// 数据文件路径（迁移数据目录后可在运行中更新）
    file_path: Arc<RwLock<PathBuf>>,
    /// 脏标记：写操作只置位，由后台线程合并落盘
    dirty: Arc<DirtySignal>,
    /// 串行化落盘：防止退出 flush 与后台线程并发写同一临时文件
    save_lock: Arc<Mutex<()>>,
    /// 持久化失败时的通知回调（GUI 下 stderr 不可见，必须让用户看到）
    notify_error: StorageErrorNotifier,
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

        Self::open_at(file_path, Self::app_error_notifier(app_handle))
    }

    /// 构造生产环境的持久化失败通知器：写入日志文件并向前端 emit `storage-error`
    fn app_error_notifier(app_handle: &AppHandle) -> StorageErrorNotifier {
        let app_handle = app_handle.clone();
        Arc::new(move |message: String| {
            // LogManager 在 Storage 之后才被 manage，这里延迟取用
            if let Some(log_manager) = app_handle.try_state::<crate::log::LogManager>() {
                let _ = log_manager.write_log(&crate::log::LogEntry {
                    r#type: "storage".to_string(),
                    message: message.clone(),
                    details: None,
                    timestamp: chrono::Local::now().to_rfc3339(),
                });
            }
            let _ = tauri::Emitter::emit(&app_handle, "storage-error", message);
        })
    }

    /// 按给定数据文件路径构造 Storage（`new` 解析出路径后调用；测试可直接传入临时目录路径与通知器）
    fn open_at(file_path: PathBuf, notify_error: StorageErrorNotifier) -> Result<Self, String> {
        let mut data: AppData = if file_path.exists() {
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

        // 计数器可能因手改 YAML / 从损坏备份恢复而缺失（#[serde(default)] 归零），
        // 归零后新建实体会与现有实体撞 ID，而 delete 按 id retain 会连带误删
        Self::normalize_id_counters(&mut data);

        let data = Arc::new(RwLock::new(data));
        let dirty = Arc::new(DirtySignal::new());
        let file_path = Arc::new(RwLock::new(file_path));
        let save_lock = Arc::new(Mutex::new(()));

        // 后台落盘线程：空闲时阻塞在条件变量上；被写操作唤醒后先等一个合并窗口，
        // 把窗口内的连续写操作合并成一次全量序列化 + 写盘
        {
            let data = Arc::clone(&data);
            let dirty = Arc::clone(&dirty);
            let file_path = Arc::clone(&file_path);
            let save_lock = Arc::clone(&save_lock);
            let notify_error = Arc::clone(&notify_error);
            std::thread::spawn(move || loop {
                dirty.wait_until_dirty();
                std::thread::sleep(SAVE_DEBOUNCE);
                // 先取走标记再落盘：落盘期间的新写入会重新置脏，下一轮再写
                dirty.take();
                let _guard = save_lock.lock();
                if let Err(e) = Self::save_to_disk(&data, &file_path) {
                    notify_error(format!("配置保存失败：{}", e));
                    // 落盘失败时重新置脏，下个合并窗口后重试
                    dirty.mark();
                }
            });
        }

        Ok(Self {
            data,
            file_path,
            dirty,
            save_lock,
            notify_error,
        })
    }

    /// 将各 ID 计数器校正到不小于现有实体的最大 ID
    ///
    /// 注意：`next_*_id` 语义是"最后分配出去的 ID"（create 时先自增再取值），
    /// 因此校正目标是 `max(计数器, 现有最大 ID)`，下一次 create 得到的即为最大 ID + 1。
    fn normalize_id_counters(data: &mut AppData) {
        fn max_id<T>(items: &[T], id_of: impl Fn(&T) -> Option<i64>) -> i64 {
            items.iter().filter_map(id_of).max().unwrap_or(0)
        }

        data.next_server_id = data.next_server_id.max(max_id(&data.servers, |s| s.id));
        data.next_subscription_id = data
            .next_subscription_id
            .max(max_id(&data.subscriptions, |s| s.id));
        data.next_template_id = data.next_template_id.max(max_id(&data.templates, |t| t.id));
        data.next_script_id = data.next_script_id.max(max_id(&data.scripts, |s| s.id));
        data.next_env_variable_id = data
            .next_env_variable_id
            .max(max_id(&data.env_variables, |e| e.id));
    }

    /// 获取当前数据文件路径
    pub fn get_file_path(&self) -> PathBuf {
        self.file_path.read().clone()
    }

    /// 更新数据文件路径（数据目录迁移后调用，后续落盘写入新位置）
    pub fn set_file_path(&self, path: PathBuf) {
        *self.file_path.write() = path;
    }

    /// 标记数据已修改并唤醒后台落盘线程
    fn mark_dirty(&self) {
        self.dirty.mark();
    }

    /// 立即将内存数据写入磁盘（应用退出前调用）
    ///
    /// 这里**不检查脏标记**：后台线程是先取走标记再写盘，
    /// 若恰好在取走之后、写完之前退出，按脏标记判断会跳过落盘导致最后一次修改丢失。
    /// 退出场景多写一次的成本可忽略，故无条件落盘。
    pub fn flush(&self) {
        let _guard = self.save_lock.lock();
        if let Err(e) = Self::save_to_disk(&self.data, &self.file_path) {
            (self.notify_error)(format!("退出时保存配置失败：{}", e));
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
    ///
    /// rename 之前必须 `sync_all`：否则断电时可能出现"目录项已指向新文件、
    /// 而文件内容仍在页缓存里"的空文件/半截文件。
    fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
        use std::io::Write;

        let tmp = path.with_extension("yaml.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}
