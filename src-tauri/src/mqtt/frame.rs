//! Rust → 前端的消息帧编码
//!
//! 一批接收到的消息编成一段小端二进制，经 Tauri `Channel` 以原始字节送到前端，
//! 不再经过 JSON 与 base64（前端解码见 `src/utils/messageFrame.ts`，两侧布局必须一致）。
//!
//! ```text
//! 批头:   u32 count
//! 每条:   f64 server_id | f64 timestamp_ms | u32 original_length
//!         | u8 qos | u8 flags (bit0 = retain, bit1 = truncated)
//!         | u16 topic_len | u32 payload_len
//!         | topic 字节 (UTF-8) | payload 字节
//! ```
//!
//! server_id 与时间戳以 f64 传输：前端 `DataView` 直接读成 Number，不需要 BigInt，
//! 整数在 2^53 以内无精度损失。topic 长度用 u16 是因为 MQTT 规范上限即 65535 字节。

use bytes::Bytes;

/// 单条消息定长头部的字节数
pub const MESSAGE_HEADER_SIZE: usize = 8 + 8 + 4 + 1 + 1 + 2 + 4;
/// 批头字节数
pub const BATCH_HEADER_SIZE: usize = 4;

/// 等待攒批发送的一条接收消息
pub struct PendingMessage {
    pub server_id: i64,
    pub topic: String,
    /// 直接持有 rumqttc 的 `Bytes` 切片，编码前不做拷贝
    pub payload: Bytes,
    pub qos: u8,
    pub retain: bool,
    /// Unix 毫秒时间戳
    pub timestamp: i64,
    /// 原始 payload 字节数（截断时前端据此提示）
    pub original_length: usize,
    /// payload 是否被截断
    pub truncated: bool,
}

impl PendingMessage {
    /// 该消息编码后占用的字节数
    pub fn encoded_size(&self) -> usize {
        MESSAGE_HEADER_SIZE + self.topic.len() + self.payload.len()
    }
}

/// 把一批消息编成一段二进制帧
pub fn encode_batch(batch: &[PendingMessage]) -> Vec<u8> {
    let total = BATCH_HEADER_SIZE + batch.iter().map(PendingMessage::encoded_size).sum::<usize>();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&(batch.len() as u32).to_le_bytes());
    for msg in batch {
        buf.extend_from_slice(&(msg.server_id as f64).to_le_bytes());
        buf.extend_from_slice(&(msg.timestamp as f64).to_le_bytes());
        buf.extend_from_slice(&(msg.original_length as u32).to_le_bytes());
        buf.push(msg.qos);
        buf.push((msg.retain as u8) | ((msg.truncated as u8) << 1));
        buf.extend_from_slice(&(msg.topic.len() as u16).to_le_bytes());
        buf.extend_from_slice(&(msg.payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(msg.topic.as_bytes());
        buf.extend_from_slice(&msg.payload);
    }
    debug_assert_eq!(buf.len(), total);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(server_id: i64, topic: &str, payload: &[u8]) -> PendingMessage {
        PendingMessage {
            server_id,
            topic: topic.to_string(),
            payload: Bytes::copy_from_slice(payload),
            qos: 1,
            retain: true,
            timestamp: 1024,
            original_length: payload.len(),
            truncated: false,
        }
    }

    /// 与前端 `messageFrame.spec.ts` 共用的固定样本：任一侧改动布局都会让对方的测试失败
    #[test]
    fn encodes_known_fixture_byte_for_byte() {
        let frame = encode_batch(&[message(1, "t/a", b"hello")]);
        let expected: Vec<u8> = vec![
            // count = 1
            0x01, 0x00, 0x00, 0x00,
            // server_id = 1.0 (f64 LE)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x3F,
            // timestamp = 1024.0 (f64 LE)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x40,
            // original_length = 5
            0x05, 0x00, 0x00, 0x00,
            // qos = 1, flags = retain
            0x01, 0x01,
            // topic_len = 3, payload_len = 5
            0x03, 0x00, 0x05, 0x00, 0x00, 0x00,
            // "t/a"
            0x74, 0x2F, 0x61,
            // "hello"
            0x68, 0x65, 0x6C, 0x6C, 0x6F,
        ];
        assert_eq!(frame, expected);
    }

    #[test]
    fn truncated_flag_and_multiple_messages_are_laid_out_sequentially() {
        let mut second = message(7, "x", b"");
        second.retain = false;
        second.truncated = true;
        second.original_length = 99;
        let frame = encode_batch(&[message(1, "t/a", b"hello"), second]);

        assert_eq!(&frame[..4], &[0x02, 0x00, 0x00, 0x00]);
        let second_start = BATCH_HEADER_SIZE + MESSAGE_HEADER_SIZE + 3 + 5;
        assert_eq!(frame.len(), second_start + MESSAGE_HEADER_SIZE + 1);
        let header = &frame[second_start..second_start + MESSAGE_HEADER_SIZE];
        assert_eq!(f64::from_le_bytes(header[0..8].try_into().unwrap()), 7.0);
        assert_eq!(u32::from_le_bytes(header[16..20].try_into().unwrap()), 99);
        assert_eq!(header[21], 0b10, "flags 应只置 truncated 位");
        assert_eq!(u16::from_le_bytes(header[22..24].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(header[24..28].try_into().unwrap()), 0);
    }

    #[test]
    fn empty_batch_is_just_a_zero_count() {
        assert_eq!(encode_batch(&[]), vec![0, 0, 0, 0]);
    }
}
