import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import {
  emitMessageBatch,
  emitRawMessageFrame,
  invokeCountOf,
  invokeMock,
  resetTauriMock,
} from "./tauriMock";
import type { ReceivedMessage } from "@/utils/messageFrame";

vi.mock("@tauri-apps/api/core", async () => {
  const { invokeMock, ChannelMock } = await import("./tauriMock");
  return { invoke: invokeMock, Channel: ChannelMock };
});
vi.mock("@tauri-apps/api/event", async () => {
  const { listenMock } = await import("./tauriMock");
  return { listen: listenMock };
});
vi.mock("element-plus", () => ({
  ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
  ElNotification: vi.fn(),
}));

import { useMqttStore } from "@/stores/mqtt";

const SERVER_ID = 1;
// 消息攒批间隔为 50ms，等待窗口取其两倍余量
const FLUSH_WAIT_MS = 120;

function message(text: string, overrides: Partial<ReceivedMessage> = {}): ReceivedMessage {
  return {
    server_id: SERVER_ID,
    topic: "test/topic",
    payload: new TextEncoder().encode(text),
    qos: 0,
    retain: false,
    timestamp: Date.now(),
    original_length: text.length,
    truncated: false,
    ...overrides,
  };
}

/** 声明了 1 条消息但连头部都不完整的坏帧 */
function brokenFrame(): ArrayBuffer {
  return new Uint8Array([1, 0, 0, 0, 9, 9, 9]).buffer;
}

function waitFlush() {
  return new Promise((resolve) => setTimeout(resolve, FLUSH_WAIT_MS));
}

function texts(payloads: (Uint8Array | undefined)[]) {
  const decoder = new TextDecoder();
  return payloads.map((p) => (p ? decoder.decode(p) : ""));
}

describe("消息接收链路", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "list_env_variables") return [];
      return undefined;
    });
  });

  it("同一批消息按到达顺序倒序插入（最新在上）", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitMessageBatch([message("m1"), message("m2"), message("m3")]);
    await waitFlush();

    expect(texts(store.getServerMessages(SERVER_ID).map((m) => m.payload))).toEqual([
      "m3",
      "m2",
      "m1",
    ]);
  });

  it("帧里的字段与二进制 payload 原样进入消息对象", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitMessageBatch([
      message("", {
        topic: "设备/1/状态",
        payload: new Uint8Array([0x00, 0xff, 0x80]),
        qos: 2,
        retain: true,
        truncated: true,
        original_length: 70000,
        timestamp: 1_700_000_000_000,
      }),
    ]);
    await waitFlush();

    const [msg] = store.getServerMessages(SERVER_ID);
    expect(msg.direction).toBe("receive");
    expect(msg.topic).toBe("设备/1/状态");
    expect(Array.from(msg.payload!)).toEqual([0x00, 0xff, 0x80]);
    expect(msg.qos).toBe(2);
    expect(msg.retain).toBe(true);
    expect(msg.truncated).toBe(true);
    expect(msg.originalLength).toBe(70000);
    expect(msg.timestamp).toBe(1_700_000_000_000);
    expect(msg.id).toBeTypeOf("number");
  });

  it("损坏的帧整帧丢弃，后续批次的消息仍能进入列表", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitMessageBatch([message("ok1")]);
    emitRawMessageFrame(brokenFrame());
    emitMessageBatch([message("ok2")]);
    await waitFlush();

    const received = texts(store.getServerMessages(SERVER_ID).map((m) => m.payload));
    expect(received).toEqual(["ok2", "ok1"]);
  });

  it("快路径与慢路径共存时跨批次顺序正确", async () => {
    const store = useMqttStore();
    await store.initListeners();

    // 第一批走慢路径（脚本缓存尚未建立）
    emitMessageBatch([message("a1"), message("a2")]);
    await waitFlush();

    // 第二批走同步快路径（缓存已确定无接收脚本）
    emitMessageBatch([message("b1"), message("b2")]);
    await waitFlush();

    expect(texts(store.getServerMessages(SERVER_ID).map((m) => m.payload))).toEqual([
      "b2",
      "b1",
      "a2",
      "a1",
    ]);
  });

  it("脚本缓存建立后持续收消息不再重复拉取脚本", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitMessageBatch([message("warm")]);
    await waitFlush();
    const after = invokeCountOf("get_enabled_scripts");
    expect(after).toBe(1);

    for (let i = 0; i < 5; i++) {
      emitMessageBatch([message(`m${i}`)]);
      await waitFlush();
    }
    expect(invokeCountOf("get_enabled_scripts")).toBe(after);
    expect(store.getServerMessages(SERVER_ID)).toHaveLength(6);
  });

  it("每次 flush 都递增 messagesVersion，供列表感知同一数组的就地变化", async () => {
    const store = useMqttStore();
    await store.initListeners();
    const before = store.messagesVersion;

    emitMessageBatch([message("v1")]);
    await waitFlush();
    expect(store.messagesVersion).toBe(before + 1);

    const listBefore = store.getServerMessages(SERVER_ID);
    emitMessageBatch([message("v2")]);
    await waitFlush();
    expect(store.messagesVersion).toBe(before + 2);
    // 数组实例不变（就地更新），靠版本号通知
    expect(store.getServerMessages(SERVER_ID)).toBe(listBefore);
  });

  it("接收脚本改写后的内容直接作为解码文本缓存", async () => {
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "get_enabled_scripts") {
        return args.scriptType === "after_receive"
          ? [
              {
                id: 1,
                server_id: SERVER_ID,
                name: "大写",
                script_type: "after_receive",
                code: "function process(p) { return p.toUpperCase(); }",
                enabled: true,
              },
            ]
          : [];
      }
      if (cmd === "list_env_variables") return [];
      return undefined;
    });

    const store = useMqttStore();
    await store.initListeners();
    emitMessageBatch([message("abc")]);
    await waitFlush();

    const [msg] = store.getServerMessages(SERVER_ID);
    expect(new TextDecoder().decode(msg.payload)).toBe("ABC");
    expect(msg.decodedText).toBe("ABC");
  });
});
