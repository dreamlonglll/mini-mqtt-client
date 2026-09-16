/**
 * Rust → 前端消息帧的编解码
 *
 * 后端把一批接收到的消息编成一段小端二进制，经 Tauri Channel 以原始字节推送，
 * 不再经过 JSON 与 base64。布局与 `src-tauri/src/mqtt/frame.rs` 保持一致，
 * 两侧各有一份基于同一固定样本的测试，任一侧改动布局都会让对方的测试失败：
 *
 * ```text
 * 批头:   u32 count
 * 每条:   f64 server_id | f64 timestamp_ms | u32 original_length
 *         | u8 qos | u8 flags (bit0 = retain, bit1 = truncated)
 *         | u16 topic_len | u32 payload_len
 *         | topic 字节 (UTF-8) | payload 字节
 * ```
 *
 * payload 以 `subarray` 视图返回，不做拷贝；一批消息共享同一段底层缓冲，
 * 直到这一批全部被列表裁掉后才随之释放。
 */
import { utf8Decoder, utf8Encoder } from "./encoding";

/** 后端推送的一条接收消息（帧解码后的形态） */
export interface ReceivedMessage {
  server_id: number;
  topic: string;
  payload: Uint8Array;
  qos: number;
  retain: boolean;
  /** Unix 毫秒时间戳 */
  timestamp: number;
  /** 原始 payload 字节数（被截断时大于 payload.length） */
  original_length: number;
  /** payload 是否被后端截断 */
  truncated: boolean;
}

/** 单条消息定长头部的字节数 */
export const MESSAGE_HEADER_SIZE = 8 + 8 + 4 + 1 + 1 + 2 + 4;
/** 批头字节数 */
export const BATCH_HEADER_SIZE = 4;

const FLAG_RETAIN = 1;
const FLAG_TRUNCATED = 2;

/**
 * 解码一帧为消息数组
 *
 * 帧不完整或长度字段越界时抛错；调用方应把整帧视为损坏丢弃，而不是部分采用。
 */
export function decodeMessageFrames(data: ArrayBuffer | Uint8Array): ReceivedMessage[] {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  const total = bytes.byteLength;
  if (total < BATCH_HEADER_SIZE) {
    throw new Error(`消息帧不完整：长度 ${total} 字节`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, total);
  const count = view.getUint32(0, true);
  const messages: ReceivedMessage[] = new Array(count);

  let offset = BATCH_HEADER_SIZE;
  for (let i = 0; i < count; i++) {
    if (offset + MESSAGE_HEADER_SIZE > total) {
      throw new Error(`消息帧不完整：第 ${i + 1} 条消息头部越界`);
    }
    const server_id = view.getFloat64(offset, true);
    const timestamp = view.getFloat64(offset + 8, true);
    const original_length = view.getUint32(offset + 16, true);
    const qos = view.getUint8(offset + 20);
    const flags = view.getUint8(offset + 21);
    const topicLength = view.getUint16(offset + 22, true);
    const payloadLength = view.getUint32(offset + 24, true);
    offset += MESSAGE_HEADER_SIZE;

    if (offset + topicLength + payloadLength > total) {
      throw new Error(`消息帧不完整：第 ${i + 1} 条消息内容越界`);
    }
    const topic = utf8Decoder.decode(bytes.subarray(offset, offset + topicLength));
    offset += topicLength;
    const payload = bytes.subarray(offset, offset + payloadLength);
    offset += payloadLength;

    messages[i] = {
      server_id,
      topic,
      payload,
      qos,
      retain: (flags & FLAG_RETAIN) !== 0,
      timestamp,
      original_length,
      truncated: (flags & FLAG_TRUNCATED) !== 0,
    };
  }
  return messages;
}

/**
 * 把消息数组编成一帧
 *
 * 生产路径只解码；编码供测试与 mock 构造后端推送的帧。
 */
export function encodeMessageFrames(messages: ReceivedMessage[]): ArrayBuffer {
  const topics = messages.map((m) => utf8Encoder.encode(m.topic));
  let total = BATCH_HEADER_SIZE;
  for (let i = 0; i < messages.length; i++) {
    total += MESSAGE_HEADER_SIZE + topics[i].byteLength + messages[i].payload.byteLength;
  }

  const buffer = new ArrayBuffer(total);
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  view.setUint32(0, messages.length, true);

  let offset = BATCH_HEADER_SIZE;
  for (let i = 0; i < messages.length; i++) {
    const m = messages[i];
    const topic = topics[i];
    view.setFloat64(offset, m.server_id, true);
    view.setFloat64(offset + 8, m.timestamp, true);
    view.setUint32(offset + 16, m.original_length, true);
    view.setUint8(offset + 20, m.qos);
    view.setUint8(offset + 21, (m.retain ? FLAG_RETAIN : 0) | (m.truncated ? FLAG_TRUNCATED : 0));
    view.setUint16(offset + 22, topic.byteLength, true);
    view.setUint32(offset + 24, m.payload.byteLength, true);
    offset += MESSAGE_HEADER_SIZE;
    bytes.set(topic, offset);
    offset += topic.byteLength;
    bytes.set(m.payload, offset);
    offset += m.payload.byteLength;
  }
  return buffer;
}
