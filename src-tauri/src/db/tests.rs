//! Storage 的集成式单元测试
//!
//! 统一通过公开方法 + 临时目录里的真实数据文件验证行为，
//! 不断言内部字段与调用次数（唯一的例外是脏标记，它是"是否触发落盘"的可观察代理）。

use super::models::{
    CreateTemplateRequest, MqttServer, Subscription, UpdateEnvVariableRequest,
    UpdateScriptRequest, UpdateSubscriptionRequest, UpdateTemplateRequest,
};
use super::Storage;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// 在临时目录里准备一个数据文件路径（TempDir 需由调用方持有，drop 后目录被清理）
fn temp_data_path() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("创建临时目录失败");
    let path = dir.path().join("data.yaml");
    (dir, path)
}

/// 忽略持久化失败通知的 Storage（大部分用例不关心通知）
fn open(path: PathBuf) -> Storage {
    Storage::open_at(path, Arc::new(|_| {})).unwrap()
}

/// 把持久化失败消息收集到共享 Vec 的 Storage
fn open_recording(path: PathBuf) -> (Storage, Arc<parking_lot::Mutex<Vec<String>>>) {
    let recorded: Arc<parking_lot::Mutex<Vec<String>>> = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let sink = Arc::clone(&recorded);
    let storage = Storage::open_at(path, Arc::new(move |msg| sink.lock().push(msg))).unwrap();
    (storage, recorded)
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
    let storage = open(path.clone());

    let keep_id = storage.create_server(sample_server("保留的服务器")).unwrap();
    let drop_id = storage.create_server(sample_server("待删除的服务器")).unwrap();

    let mut updated = storage.get_server(keep_id).unwrap();
    updated.name = "保留的服务器-已改名".to_string();
    updated.port = 8883;
    storage.update_server(updated).unwrap();
    storage.delete_server(drop_id).unwrap();

    storage.flush();
    drop(storage);

    let reloaded = open(path);
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
    let storage = open(path.clone());

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

    let reloaded = open(path);
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
    let storage = open(path.clone());
    let first_id = storage.create_server(sample_server("第一个")).unwrap();
    storage.delete_server(first_id).unwrap();
    storage.flush();
    drop(storage);

    let reloaded = open(path);
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
    let storage = open(path.clone());
    storage.create_server(sample_server("退出前的最后一次修改")).unwrap();

    // 第一次 flush 后脏标记已被消费，等价于"后台线程刚 swap 完"的状态
    storage.flush();
    assert!(path.exists());

    // 删掉文件模拟"脏标记已取走但写盘尚未完成"，此时退出前的 flush 必须重新落盘
    fs::remove_file(&path).unwrap();
    storage.flush();
    assert!(path.exists(), "flush() 检查脏标记会跳过落盘，导致最后一次修改丢失");

    let reloaded = open(path);
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

    let storage = open(path.clone());
    assert_eq!(server_count(&storage), 0, "损坏文件应以空数据启动");

    let backups = corrupt_backups(dir.path());
    assert_eq!(backups.len(), 1, "损坏文件应被备份为 data.corrupt-*，实际：{:?}", backups);
    let backup_content = fs::read_to_string(dir.path().join(&backups[0])).unwrap();
    assert_eq!(backup_content, corrupt, "备份内容应与损坏原文一致");

    // 损坏恢复后仍可正常写入并落盘
    storage.create_server(sample_server("恢复后新建")).unwrap();
    storage.flush();
    let reloaded = open(path);
    assert_eq!(server_count(&reloaded), 1);
}

/// 手改 YAML / 从损坏备份恢复时计数器可能缺失（#[serde(default)] 归零），
/// 加载后必须按现有实体的最大 ID 校正，否则新建实体会与现有实体撞 ID，
/// 而 delete 是按 id retain，会连带误删。
#[test]
fn id_counters_are_corrected_from_existing_entities_on_load() {
    let (_dir, path) = temp_data_path();
    // 只有实体、没有任何 next_*_id 字段的数据文件
    let hand_written = r#"
servers:
  - id: 7
    name: 手工恢复的服务器
    host: 127.0.0.1
    port: 1883
    protocol_version: 3.1.1
    username: null
    password: null
    client_id: null
    keep_alive: 60
    clean_session: true
    use_tls: false
    ca_cert: null
    client_cert: null
    client_key: null
    client_key_password: null
    created_at: null
    updated_at: null
subscriptions:
  - id: 3
    server_id: 7
    topic: a/b
    qos: 0
    is_active: true
    color: null
    created_at: null
templates: []
scripts: []
env_variables: []
"#;
    fs::write(&path, hand_written).unwrap();

    let storage = open(path);
    assert_eq!(server_count(&storage), 1, "手写数据应被正常读取");

    let new_id = storage.create_server(sample_server("新建服务器")).unwrap();
    assert_ne!(new_id, 7, "新建的 Server ID 不能与现有实体冲突");
    assert!(new_id > 7, "新建的 Server ID 应大于现有最大 ID，实际：{}", new_id);

    let new_sub = storage
        .create_subscription(Subscription {
            id: None,
            server_id: 7,
            topic: "c/d".to_string(),
            qos: 0,
            is_active: true,
            color: None,
            created_at: None,
        })
        .unwrap();
    assert!(
        new_sub.id.unwrap() > 3,
        "新建订阅 ID 应大于现有最大 ID，实际：{:?}",
        new_sub.id
    );

    // 删除新建的 Server 不应误删同 ID 的旧数据
    storage.delete_server(new_id).unwrap();
    assert!(storage.get_server(7).is_some(), "删除新建实体时误删了现有实体");
}

#[test]
fn update_of_missing_id_returns_error() {
    let (_dir, path) = temp_data_path();
    let storage = open(path);

    let mut ghost = sample_server("不存在的服务器");
    ghost.id = Some(999);
    assert!(storage.update_server(ghost).is_err(), "update_server 目标不存在应返回错误");

    assert!(
        storage.update_subscription_status(999, false).is_err(),
        "update_subscription_status 目标不存在应返回错误"
    );
    assert!(
        storage
            .update_subscription(UpdateSubscriptionRequest {
                id: 999,
                topic: Some("a/b".to_string()),
                qos: None,
                color: None,
            })
            .is_err(),
        "update_subscription 目标不存在应返回错误"
    );
    assert!(
        storage
            .update_template(UpdateTemplateRequest {
                id: 999,
                name: Some("x".to_string()),
                topic: None,
                payload: None,
                payload_type: None,
                qos: None,
                retain: None,
                description: None,
                category: None,
            })
            .is_err(),
        "update_template 目标不存在应返回错误"
    );
    assert!(
        storage
            .update_script(UpdateScriptRequest {
                id: 999,
                name: None,
                code: Some("x".to_string()),
                enabled: None,
                description: None,
            })
            .is_err(),
        "update_script 目标不存在应返回错误"
    );
    assert!(
        storage.toggle_script(999, true).is_err(),
        "toggle_script 目标不存在应返回错误"
    );
    assert!(
        storage
            .update_env_variable(UpdateEnvVariableRequest {
                id: 999,
                name: None,
                value: Some("x".to_string()),
                description: None,
            })
            .is_err(),
        "update_env_variable 目标不存在应返回错误"
    );
}

/// 目标不存在的 update 不应置脏标记（否则会触发一次无意义的全量落盘）
#[test]
fn failed_update_does_not_mark_dirty() {
    let (_dir, path) = temp_data_path();
    let storage = open(path);
    storage.dirty.clear();

    let mut ghost = sample_server("不存在的服务器");
    ghost.id = Some(999);
    let _ = storage.update_server(ghost);

    assert!(!storage.dirty.is_dirty(), "update 失败时不应置脏标记");
}

/// 写操作后后台线程应在合并窗口后自行落盘，无需调用 flush
///
/// 落盘线程改为条件变量唤醒后，这里保证"置脏 → 唤醒 → 落盘"这条链没有断。
#[test]
fn background_thread_persists_after_debounce_window() {
    let (_dir, path) = temp_data_path();
    let storage = open(path.clone());
    storage.create_server(sample_server("由后台线程落盘")).unwrap();
    assert!(!path.exists(), "合并窗口内不应立即落盘");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(path.exists(), "后台线程应在合并窗口后落盘");

    let reloaded = open(path);
    assert_eq!(reloaded.get_server(1).unwrap().name, "由后台线程落盘");
}

/// created_at 由服务端维护，客户端传来的值不得覆盖
#[test]
fn update_server_preserves_created_at() {
    let (_dir, path) = temp_data_path();
    let storage = open(path);
    let id = storage.create_server(sample_server("原始服务器")).unwrap();
    let original_created_at = storage.get_server(id).unwrap().created_at.unwrap();

    let mut modified = storage.get_server(id).unwrap();
    modified.name = "改名后的服务器".to_string();
    modified.created_at = Some("1970-01-01T00:00:00+00:00".to_string());
    storage.update_server(modified).unwrap();

    let after = storage.get_server(id).unwrap();
    assert_eq!(after.name, "改名后的服务器");
    assert_eq!(
        after.created_at.unwrap(),
        original_created_at,
        "created_at 不应被客户端覆盖"
    );
}

/// 持久化失败必须走通知回调（GUI 下 eprintln 不可见）
#[test]
fn persist_failure_is_reported_through_notifier() {
    let dir = TempDir::new().unwrap();
    // 父目录不存在 → 写临时文件必然失败
    let bad_path = dir.path().join("不存在的子目录").join("data.yaml");
    let (storage, recorded) = open_recording(bad_path);

    storage.create_server(sample_server("写不进去的数据")).unwrap();
    storage.flush();

    let messages = recorded.lock().clone();
    assert!(!messages.is_empty(), "持久化失败应触发通知回调");
    assert!(
        messages[0].chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
        "通知消息应为中文：{}",
        messages[0]
    );
}
