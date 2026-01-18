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
use tauri::Manager;

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Server 命令
            get_servers,
            get_server,
            create_server,
            update_server,
            delete_server,
            // MQTT 命令
            mqtt_connect,
            mqtt_disconnect,
            mqtt_publish,
            mqtt_subscribe,
            mqtt_unsubscribe,
            mqtt_is_connected,
            // 订阅命令
            add_subscription,
            remove_subscription,
            get_subscriptions,
            toggle_subscription,
            update_subscription,
            // 消息命令
            publish_message,
            get_message_history,
            clear_message_history,
            // 模板命令
            create_template,
            get_template,
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
            get_script,
            get_enabled_scripts,
            create_script,
            update_script,
            delete_script,
            toggle_script,
            // 日志命令
            write_error_log,
            get_recent_logs,
            get_log_dir,
            clear_logs,
            // 环境变量命令
            list_env_variables,
            get_env_variable,
            create_env_variable,
            update_env_variable,
            delete_env_variable,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
