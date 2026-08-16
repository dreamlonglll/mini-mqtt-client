//! 订阅实体的存储操作

use super::models::{Subscription, UpdateSubscriptionRequest};
use super::Storage;

impl Storage {
    pub fn get_subscriptions_json(&self, server_id: i64) -> Result<serde_json::Value, String> {
        let data = self.data.read();
        let items: Vec<&Subscription> = data
            .subscriptions
            .iter()
            .filter(|s| s.server_id == server_id)
            .collect();
        serde_json::to_value(items).map_err(|e| e.to_string())
    }

    pub fn create_subscription(&self, mut sub: Subscription) -> Result<Subscription, String> {
        let mut data = self.data.write();
        data.next_subscription_id += 1;
        sub.id = Some(data.next_subscription_id);
        sub.created_at = Some(chrono::Utc::now().to_rfc3339());
        let result = sub.clone();
        data.subscriptions.push(sub);
        drop(data);
        self.mark_dirty();
        Ok(result)
    }

    pub fn update_subscription_status(&self, id: i64, is_active: bool) -> Result<(), String> {
        let mut data = self.data.write();
        match data.subscriptions.iter_mut().find(|s| s.id == Some(id)) {
            Some(sub) => sub.is_active = is_active,
            None => return Err("订阅不存在".to_string()),
        }
        drop(data);
        self.mark_dirty();
        Ok(())
    }

    pub fn update_subscription(&self, req: UpdateSubscriptionRequest) -> Result<Subscription, String> {
        let mut data = self.data.write();
        if let Some(sub) = data.subscriptions.iter_mut().find(|s| s.id == Some(req.id)) {
            if let Some(topic) = req.topic {
                sub.topic = topic;
            }
            if let Some(qos) = req.qos {
                sub.qos = qos;
            }
            // color 可以设置为 None（清除颜色）
            sub.color = req.color;
            let result = sub.clone();
            drop(data);
            self.mark_dirty();
            Ok(result)
        } else {
            Err("Subscription not found".to_string())
        }
    }

    pub fn delete_subscription(&self, id: i64) -> Result<(), String> {
        let mut data = self.data.write();
        data.subscriptions.retain(|s| s.id != Some(id));
        drop(data);
        self.mark_dirty();
        Ok(())
    }
}
