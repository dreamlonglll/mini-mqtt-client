//! 消息历史的存储操作
//!
//! 消息历史只保留在内存（按 server 分桶），不写入 data.yaml，
//! 因此这里的操作都不置脏标记。

use super::models::MessageHistory;
use super::Storage;
use std::sync::atomic::Ordering;

/// 每个 server 内存中保留的最大消息条数
const MAX_MESSAGES_PER_SERVER: usize = 1000;
/// 发布历史中单条 payload 保留的最大字节数（与接收方向的截断策略一致）
pub(super) const MAX_HISTORY_PAYLOAD: usize = 64 * 1024;

/// 截断过大的发布历史 payload
///
/// 反复发布大 payload 会让常驻内存持续膨胀；超限时保留前 64KB 并在尾部追加标记，
/// 不新增模型字段，前后端契约保持不变。
fn truncate_history_payload(payload: String) -> String {
    if payload.len() <= MAX_HISTORY_PAYLOAD {
        return payload;
    }
    let original_len = payload.len();
    // 回退到最近的 UTF-8 字符边界，避免切出非法字符串
    let mut end = MAX_HISTORY_PAYLOAD;
    while end > 0 && !payload.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = payload;
    truncated.truncate(end);
    truncated.push_str(&format!("...[已截断，原始 {} 字节]", original_len));
    truncated
}

impl Storage {
    pub fn get_messages(&self, server_id: i64, limit: usize, offset: usize) -> Vec<MessageHistory> {
        let messages = self.messages.read();
        messages
            .get(&server_id)
            .map(|queue| queue.iter().rev().skip(offset).take(limit).cloned().collect())
            .unwrap_or_default()
    }

    pub fn create_message(&self, mut msg: MessageHistory) -> Result<MessageHistory, String> {
        msg.id = Some(self.next_message_id.fetch_add(1, Ordering::Relaxed) + 1);
        msg.created_at = Some(chrono::Utc::now().to_rfc3339());
        // 超大 payload 只留前 64KB + 截断标记，防止反复发布导致常驻内存膨胀
        msg.payload = msg.payload.map(truncate_history_payload);
        let result = msg.clone();

        let mut messages = self.messages.write();
        let queue = messages.entry(result.server_id).or_default();
        queue.push_back(msg);
        if queue.len() > MAX_MESSAGES_PER_SERVER {
            queue.pop_front();
        }
        Ok(result)
    }

    pub fn clear_messages(&self, server_id: i64) -> Result<(), String> {
        self.messages.write().remove(&server_id);
        Ok(())
    }
}
