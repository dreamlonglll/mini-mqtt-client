pub mod models;

use rusqlite::{Connection, Result};
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .expect("Failed to get app data dir");

        std::fs::create_dir_all(&app_dir).expect("Failed to create app data dir");

        let db_path = app_dir.join("mqtt_client.db");
        let conn = Connection::open(db_path)?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            -- MQTT Server 配置表
            CREATE TABLE IF NOT EXISTS mqtt_servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER DEFAULT 1883,
                protocol_version TEXT DEFAULT '5.0',
                username TEXT,
                password TEXT,
                client_id TEXT,
                keep_alive INTEGER DEFAULT 60,
                clean_session INTEGER DEFAULT 1,
                use_tls INTEGER DEFAULT 0,
                ca_cert TEXT,
                client_cert TEXT,
                client_key TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- 命令模板表
            CREATE TABLE IF NOT EXISTS command_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                topic TEXT NOT NULL,
                payload TEXT,
                qos INTEGER DEFAULT 0,
                retain INTEGER DEFAULT 0,
                category TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
            );

            -- 历史消息表
            CREATE TABLE IF NOT EXISTS message_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                direction TEXT NOT NULL,
                topic TEXT NOT NULL,
                payload BLOB,
                qos INTEGER DEFAULT 0,
                retain INTEGER DEFAULT 0,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
            );

            -- 订阅记录表
            CREATE TABLE IF NOT EXISTS subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                topic TEXT NOT NULL,
                qos INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
            );
            ",
        )?;

        Ok(())
    }
}
