import { describe, it, expect } from "vitest";
import {
  decodeMessageFrames,
  encodeMessageFrames,
  MESSAGE_HEADER_SIZE,
  type ReceivedMessage,
} from "@/utils/messageFrame";

/** 与 Rust 侧 `frame.rs` 的 `encodes_known_fixture_byte_for_byte` 共用的固定样本 */
const RUST_FIXTURE = new Uint8Array([
  // count = 1
  0x01, 0x00, 0x00, 0x00,
  // server_id = 1.0 (f64 LE)
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f,
  // timestamp = 1024.0 (f64 LE)
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x40,
  // original_length = 5
  0x05, 0x00, 0x00, 0x00,
  // qos = 1, flags = retain
  0x01, 0x01,
  // topic_len = 3, payload_len = 5
  0x03, 0x00, 0x05, 0x00, 0x00, 0x00,
  // "t/a"
  0x74, 0x2f, 0x61,
  // "hello"
  0x68, 0x65, 0x6c, 0x6c, 0x6f,
]);

function message(overrides: Partial<ReceivedMessage> = {}): ReceivedMessage {
  return {
    server_id: 1,
    topic: "t/a",
    payload: new TextEncoder().encode("hello"),
    qos: 1,
    retain: true,
    timestamp: 1024,
    original_length: 5,
    truncated: false,
    ...overrides,
  };
}

describe("消息帧编解码", () => {
  it("解码 Rust 侧生成的固定样本", () => {
    const [msg] = decodeMessageFrames(RUST_FIXTURE.buffer);
    expect(msg.server_id).toBe(1);
    expect(msg.timestamp).toBe(1024);
    expect(msg.original_length).toBe(5);
    expect(msg.qos).toBe(1);
    expect(msg.retain).toBe(true);
    expect(msg.truncated).toBe(false);
    expect(msg.topic).toBe("t/a");
    expect(new TextDecoder().decode(msg.payload)).toBe("hello");
  });

  it("前端编码器与 Rust 固定样本逐字节一致", () => {
    const encoded = new Uint8Array(encodeMessageFrames([message()]));
    expect(Array.from(encoded)).toEqual(Array.from(RUST_FIXTURE));
  });

  it("多条消息、中文 topic、二进制 payload 与截断标记往返无损", () => {
    const binary = new Uint8Array([0x00, 0xff, 0x10, 0x7f]);
    const input = [
      message(),
      message({
        server_id: 7,
        topic: "设备/温度/#",
        payload: binary,
        qos: 2,
        retain: false,
        truncated: true,
        original_length: 65536 + 1,
        timestamp: 1_700_000_000_123,
      }),
      message({ topic: "", payload: new Uint8Array(), original_length: 0 }),
    ];

    const decoded = decodeMessageFrames(encodeMessageFrames(input));

    expect(decoded).toHaveLength(3);
    expect(decoded[1].server_id).toBe(7);
    expect(decoded[1].topic).toBe("设备/温度/#");
    expect(Array.from(decoded[1].payload)).toEqual(Array.from(binary));
    expect(decoded[1].qos).toBe(2);
    expect(decoded[1].retain).toBe(false);
    expect(decoded[1].truncated).toBe(true);
    expect(decoded[1].original_length).toBe(65537);
    expect(decoded[1].timestamp).toBe(1_700_000_000_123);
    expect(decoded[2].topic).toBe("");
    expect(decoded[2].payload.byteLength).toBe(0);
  });

  it("payload 是底层缓冲的视图而非拷贝", () => {
    const buffer = encodeMessageFrames([message()]);
    const [msg] = decodeMessageFrames(buffer);
    expect(msg.payload.buffer).toBe(buffer);
  });

  it("接受带偏移的 Uint8Array 视图", () => {
    const frame = new Uint8Array(encodeMessageFrames([message()]));
    const padded = new Uint8Array(frame.byteLength + 3);
    padded.set(frame, 3);
    const [msg] = decodeMessageFrames(padded.subarray(3));
    expect(msg.topic).toBe("t/a");
    expect(new TextDecoder().decode(msg.payload)).toBe("hello");
  });

  it("帧不完整时抛错而不是部分采用", () => {
    expect(() => decodeMessageFrames(new Uint8Array([1, 0]).buffer)).toThrow();
    // 声明 1 条消息但连头部都不够
    expect(() =>
      decodeMessageFrames(new Uint8Array([1, 0, 0, 0, 9, 9, 9]).buffer)
    ).toThrow();
    // 头部完整但内容长度越界
    const truncated = RUST_FIXTURE.slice(0, 4 + MESSAGE_HEADER_SIZE + 2);
    expect(() => decodeMessageFrames(truncated.buffer)).toThrow();
  });
});
