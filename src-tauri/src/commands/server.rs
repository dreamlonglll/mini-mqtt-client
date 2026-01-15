use crate::db::models::MqttServer;
use crate::db::Database;
use rusqlite::OptionalExtension;
use tauri::State;

#[tauri::command]
pub async fn get_servers(db: State<'_, Database>) -> Result<Vec<MqttServer>, String> {
    let conn = db.conn.lock().unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, protocol_version, username, password, 
             client_id, keep_alive, clean_session, use_tls, ca_cert, client_cert, 
             client_key, created_at, updated_at FROM mqtt_servers ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let servers = stmt
        .query_map([], |row| {
            Ok(MqttServer {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                protocol_version: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                client_id: row.get(7)?,
                keep_alive: row.get(8)?,
                clean_session: row.get::<_, i32>(9)? == 1,
                use_tls: row.get::<_, i32>(10)? == 1,
                ca_cert: row.get(11)?,
                client_cert: row.get(12)?,
                client_key: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(servers)
}

#[tauri::command]
pub async fn create_server(db: State<'_, Database>, server: MqttServer) -> Result<i64, String> {
    let conn = db.conn.lock().unwrap();

    conn.execute(
        "INSERT INTO mqtt_servers (name, host, port, protocol_version, username, password, 
         client_id, keep_alive, clean_session, use_tls, ca_cert, client_cert, client_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![
            server.name,
            server.host,
            server.port,
            server.protocol_version,
            server.username,
            server.password,
            server.client_id,
            server.keep_alive,
            server.clean_session as i32,
            server.use_tls as i32,
            server.ca_cert,
            server.client_cert,
            server.client_key,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub async fn update_server(db: State<'_, Database>, server: MqttServer) -> Result<(), String> {
    let conn = db.conn.lock().unwrap();

    conn.execute(
        "UPDATE mqtt_servers SET 
         name = ?1, host = ?2, port = ?3, protocol_version = ?4, username = ?5, 
         password = ?6, client_id = ?7, keep_alive = ?8, clean_session = ?9, 
         use_tls = ?10, ca_cert = ?11, client_cert = ?12, client_key = ?13,
         updated_at = CURRENT_TIMESTAMP
         WHERE id = ?14",
        rusqlite::params![
            server.name,
            server.host,
            server.port,
            server.protocol_version,
            server.username,
            server.password,
            server.client_id,
            server.keep_alive,
            server.clean_session as i32,
            server.use_tls as i32,
            server.ca_cert,
            server.client_cert,
            server.client_key,
            server.id,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_server(db: State<'_, Database>, id: i64) -> Result<(), String> {
    let conn = db.conn.lock().unwrap();

    conn.execute("DELETE FROM mqtt_servers WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_server(db: State<'_, Database>, id: i64) -> Result<Option<MqttServer>, String> {
    let conn = db.conn.lock().unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, protocol_version, username, password, 
             client_id, keep_alive, clean_session, use_tls, ca_cert, client_cert, 
             client_key, created_at, updated_at FROM mqtt_servers WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let server = stmt
        .query_row([id], |row| {
            Ok(MqttServer {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                protocol_version: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                client_id: row.get(7)?,
                keep_alive: row.get(8)?,
                clean_session: row.get::<_, i32>(9)? == 1,
                use_tls: row.get::<_, i32>(10)? == 1,
                ca_cert: row.get(11)?,
                client_cert: row.get(12)?,
                client_key: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })
        .optional()
        .map_err(|e: rusqlite::Error| e.to_string())?;

    Ok(server)
}
