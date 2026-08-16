//! 命令模板实体的存储操作

use super::models::{CommandTemplate, CreateTemplateRequest, UpdateTemplateRequest};
use super::Storage;

impl Storage {
    pub fn get_templates_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&CommandTemplate> = data
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn get_template(&self, id: i64) -> Option<CommandTemplate> {
        let data = self.data.read();
        data.templates.iter().find(|t| t.id == Some(id)).cloned()
    }

    pub fn create_template(&self, req: CreateTemplateRequest) -> Result<i64, String> {
        let mut data = self.data.write();
        data.next_template_id += 1;
        let id = data.next_template_id;
        let now = chrono::Utc::now().to_rfc3339();

        let template = CommandTemplate {
            id: Some(id),
            server_id: req.server_id,
            name: req.name,
            topic: req.topic,
            payload: req.payload,
            payload_type: req.payload_type,
            qos: req.qos,
            retain: req.retain,
            description: req.description,
            category: req.category,
            use_count: 0,
            last_used_at: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };

        data.templates.push(template);
        drop(data);
        self.mark_dirty();
        Ok(id)
    }

    pub fn update_template(&self, req: UpdateTemplateRequest) -> Result<(), String> {
        let mut data = self.data.write();
        match data.templates.iter_mut().find(|t| t.id == Some(req.id)) {
            Some(template) => {
                if let Some(name) = req.name {
                    template.name = name;
                }
                if let Some(topic) = req.topic {
                    template.topic = topic;
                }
                if let Some(payload) = req.payload {
                    template.payload = payload;
                }
                if let Some(payload_type) = req.payload_type {
                    template.payload_type = payload_type;
                }
                if let Some(qos) = req.qos {
                    template.qos = qos;
                }
                if let Some(retain) = req.retain {
                    template.retain = retain;
                }
                if let Some(description) = req.description {
                    template.description = Some(description);
                }
                if let Some(category) = req.category {
                    template.category = Some(category);
                }
                template.updated_at = Some(chrono::Utc::now().to_rfc3339());
            }
            None => return Err("模板不存在".to_string()),
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn delete_template(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.templates.retain(|t| t.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn increment_template_use_count(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        if let Some(template) = data.templates.iter_mut().find(|t| t.id == Some(id)) {
            template.use_count += 1;
            template.last_used_at = Some(chrono::Utc::now().to_rfc3339());
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn get_template_categories(&self, server_id: i64) -> Vec<String> {
        let data = self.data.read();
        let mut categories: Vec<String> = data
            .templates
            .iter()
            .filter(|t| t.server_id == server_id)
            .filter_map(|t| t.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }
}
