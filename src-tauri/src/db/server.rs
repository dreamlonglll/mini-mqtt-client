//! Server 实体的存储操作

use super::models::MqttServer;
use super::Storage;

impl Storage {
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

    pub fn update_server(&self, mut server: MqttServer) -> Result<(), String> {
        let mut data = self.data.write();
        match data.servers.iter_mut().find(|s| s.id == server.id) {
            Some(existing) => {
                // 创建时间由服务端维护，不接受客户端覆盖
                server.created_at = existing.created_at.clone();
                server.updated_at = Some(chrono::Utc::now().to_rfc3339());
                *existing = server;
            }
            None => return Err("服务器不存在".to_string()),
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
}
