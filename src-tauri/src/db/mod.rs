pub mod models;

use models::{MessageHistory, NewMessageHistory, NewSubscription, Subscription};
use rusqlite::{params, Connection, Result};
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
                payload TEXT,
                payload_format TEXT DEFAULT 'text',
                qos INTEGER DEFAULT 0,
                retain INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
            );

            -- 订阅记录表
            CREATE TABLE IF NOT EXISTS subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                topic TEXT NOT NULL,
                qos INTEGER DEFAULT 0,
                is_active INTEGER DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
            );
            ",
        )?;

        Ok(())
    }

    // 订阅相关操作
    pub fn insert_subscription(&self, sub: &NewSubscription) -> Result<Subscription> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO subscriptions (server_id, topic, qos, is_active) VALUES (?1, ?2, ?3, ?4)",
            params![sub.server_id, sub.topic, sub.qos, sub.is_active as i32],
        )?;

        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_subscription_by_id(id)
    }

    pub fn get_subscription_by_id(&self, id: i64) -> Result<Subscription> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, server_id, topic, qos, is_active, created_at FROM subscriptions WHERE id = ?1",
            params![id],
            |row| {
                Ok(Subscription {
                    id: Some(row.get(0)?),
                    server_id: row.get(1)?,
                    topic: row.get(2)?,
                    qos: row.get(3)?,
                    is_active: row.get::<_, i32>(4)? == 1,
                    created_at: row.get(5)?,
                })
            },
        )
    }

    pub fn get_subscriptions_by_server(&self, server_id: i64) -> Result<Vec<Subscription>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, server_id, topic, qos, is_active, created_at FROM subscriptions WHERE server_id = ?1 ORDER BY created_at DESC",
        )?;

        let subs = stmt.query_map(params![server_id], |row| {
            Ok(Subscription {
                id: Some(row.get(0)?),
                server_id: row.get(1)?,
                topic: row.get(2)?,
                qos: row.get(3)?,
                is_active: row.get::<_, i32>(4)? == 1,
                created_at: row.get(5)?,
            })
        })?;

        subs.collect()
    }

    pub fn update_subscription_status(&self, id: i64, is_active: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE subscriptions SET is_active = ?1 WHERE id = ?2",
            params![is_active as i32, id],
        )?;
        Ok(())
    }

    pub fn delete_subscription(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM subscriptions WHERE id = ?1", params![id])?;
        Ok(())
    }

    // 消息历史相关操作
    pub fn insert_message_history(&self, msg: &NewMessageHistory) -> Result<MessageHistory> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO message_history (server_id, topic, payload, payload_format, direction, qos, retain) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                msg.server_id,
                msg.topic,
                msg.payload,
                msg.payload_format,
                msg.direction,
                msg.qos,
                msg.retain as i32
            ],
        )?;

        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_message_history_by_id(id)
    }

    pub fn get_message_history_by_id(&self, id: i64) -> Result<MessageHistory> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, server_id, topic, payload, payload_format, direction, qos, retain, created_at FROM message_history WHERE id = ?1",
            params![id],
            |row| {
                Ok(MessageHistory {
                    id: Some(row.get(0)?),
                    server_id: row.get(1)?,
                    topic: row.get(2)?,
                    payload: row.get(3)?,
                    payload_format: row.get(4)?,
                    direction: row.get(5)?,
                    qos: row.get(6)?,
                    retain: row.get::<_, i32>(7)? == 1,
                    created_at: row.get(8)?,
                })
            },
        )
    }

    pub fn get_message_history(
        &self,
        server_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<MessageHistory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, server_id, topic, payload, payload_format, direction, qos, retain, created_at FROM message_history WHERE server_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
        )?;

        let messages = stmt.query_map(params![server_id, limit, offset], |row| {
            Ok(MessageHistory {
                id: Some(row.get(0)?),
                server_id: row.get(1)?,
                topic: row.get(2)?,
                payload: row.get(3)?,
                payload_format: row.get(4)?,
                direction: row.get(5)?,
                qos: row.get(6)?,
                retain: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
            })
        })?;

        messages.collect()
    }

    pub fn clear_message_history(&self, server_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM message_history WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(())
    }
}
