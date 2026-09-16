mod commands;
mod db;
mod log;
mod mqtt;

use commands::env::*;
use commands::log::*;
use commands::mqtt::*;
use commands::publish::*;
use commands::script::*;
use commands::server::*;
use commands::settings::*;
use commands::subscription::*;
use commands::template::*;
use db::Storage;
use log::LogManager;
use mqtt::MqttManager;
use std::time::Duration;
use tauri::Manager;

/// 前端迟迟没有调用 show 时兜底显示主窗口的等待时长
///
/// 主窗口在 tauri.conf.json 里配置为初始隐藏，由前端在主题应用完毕后再显示，
/// 避免启动时先闪一下空白页与浅色背景。若前端加载失败，超时后仍要把窗口显示出来，
/// 否则用户会以为应用没有启动。
const SHOW_WINDOW_FALLBACK: Duration = Duration::from_secs(3);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 初始化存储
            let storage =
                Storage::new(&app.handle()).expect("Failed to initialize storage");
            app.manage(storage);

            // 初始化 MQTT 管理器
            let mqtt_manager = MqttManager::new(app.handle().clone());
            app.manage(mqtt_manager);

            // 初始化日志管理器
            let log_manager =
                LogManager::new(&app.handle()).expect("Failed to initialize log manager");
            app.manage(log_manager);

            // 主窗口显示兜底
            if let Some(window) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    std::thread::sleep(SHOW_WINDOW_FALLBACK);
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Server 命令
            get_servers,
            create_server,
            update_server,
            delete_server,
            // MQTT 命令
            register_message_channel,
            mqtt_connect,
            mqtt_disconnect,
            mqtt_subscribe,
            mqtt_unsubscribe,
            // 订阅命令
            add_subscription,
            remove_subscription,
            get_subscriptions,
            toggle_subscription,
            update_subscription,
            // 消息命令
            publish_message,
            // 模板命令
            create_template,
            list_templates,
            update_template,
            delete_template,
            use_template,
            get_template_categories,
            export_templates,
            import_templates,
            duplicate_template,
            // 设置命令
            get_data_path,
            migrate_data_path,
            select_data_folder,
            // 脚本命令
            list_scripts,
            get_enabled_scripts,
            create_script,
            update_script,
            delete_script,
            toggle_script,
            // 日志命令
            write_error_logs,
            get_log_dir,
            clear_logs,
            // 环境变量命令
            list_env_variables,
            create_env_variable,
            update_env_variable,
            delete_env_variable,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // 退出前把防抖中未落盘的数据写入磁盘
                app_handle.state::<Storage>().flush();
            }
        });
}
