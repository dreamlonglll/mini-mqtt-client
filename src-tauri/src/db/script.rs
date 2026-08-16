//! 预处理脚本实体的存储操作

use super::models::{CreateScriptRequest, Script, UpdateScriptRequest};
use super::Storage;

impl Storage {
    pub fn get_scripts_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&Script> = data
            .scripts
            .iter()
            .filter(|s| s.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
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
        match data.scripts.iter_mut().find(|s| s.id == Some(req.id)) {
            Some(script) => {
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
            None => return Err("脚本不存在".to_string()),
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
        match data.scripts.iter_mut().find(|s| s.id == Some(id)) {
            Some(script) => {
                script.enabled = enabled;
                script.updated_at = Some(chrono::Utc::now().to_rfc3339());
            }
            None => return Err("脚本不存在".to_string()),
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }
}
