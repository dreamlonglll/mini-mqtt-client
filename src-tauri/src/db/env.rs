//! 环境变量实体的存储操作

use super::models::{CreateEnvVariableRequest, EnvVariable, UpdateEnvVariableRequest};
use super::Storage;

impl Storage {
    pub fn get_env_variables_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&EnvVariable> = data
            .env_variables
            .iter()
            .filter(|e| e.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn create_env_variable(&self, req: CreateEnvVariableRequest) -> Result<i64, String> {
        let mut data = self.data.write();

        // 检查变量名是否重复
        let exists = data.env_variables.iter().any(|e| {
            e.server_id == req.server_id && e.name == req.name
        });
        if exists {
            return Err("Variable name already exists".to_string());
        }

        data.next_env_variable_id += 1;
        let id = data.next_env_variable_id;
        let now = chrono::Utc::now().to_rfc3339();

        let env_var = EnvVariable {
            id: Some(id),
            server_id: req.server_id,
            name: req.name,
            value: req.value,
            description: req.description,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };

        data.env_variables.push(env_var);
        drop(data);
        self.mark_dirty();
        Ok(id)
    }

    pub fn update_env_variable(&self, req: UpdateEnvVariableRequest) -> Result<(), String> {
        let mut data = self.data.write();

        // 如果要更新名称，检查是否与其他变量重复
        if let Some(new_name) = &req.name {
            let current = data.env_variables.iter().find(|e| e.id == Some(req.id));
            if let Some(current) = current {
                let exists = data.env_variables.iter().any(|e| {
                    e.server_id == current.server_id && e.name == *new_name && e.id != Some(req.id)
                });
                if exists {
                    return Err("Variable name already exists".to_string());
                }
            }
        }

        match data.env_variables.iter_mut().find(|e| e.id == Some(req.id)) {
            Some(env_var) => {
                if let Some(name) = req.name {
                    env_var.name = name;
                }
                if let Some(value) = req.value {
                    env_var.value = value;
                }
                if let Some(description) = req.description {
                    env_var.description = Some(description);
                }
                env_var.updated_at = Some(chrono::Utc::now().to_rfc3339());
            }
            None => return Err("环境变量不存在".to_string()),
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_env_variable(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.env_variables.retain(|e| e.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }
}
