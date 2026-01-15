mod commands;
mod db;
mod mqtt;

use commands::mqtt::*;
use commands::publish::*;
use commands::server::*;
use commands::subscription::*;
use commands::template::*;
use db::Storage;
use mqtt::MqttManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化存储
            let storage =
                Storage::new(&app.handle()).expect("Failed to initialize storage");
            app.manage(storage);

            // 初始化 MQTT 管理器
            let mqtt_manager = MqttManager::new(app.handle().clone());
            app.manage(mqtt_manager);

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
