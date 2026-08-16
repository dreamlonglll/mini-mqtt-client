import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { emitTauriEvent, invokeMock, resetTauriMock } from "./tauriMock";

vi.mock("@tauri-apps/api/core", async () => {
  const { invokeMock } = await import("./tauriMock");
  return { invoke: invokeMock };
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

function message(text: string) {
  return {
    server_id: SERVER_ID,
    topic: "test/topic",
    payload: btoa(text),
    qos: 0,
    retain: false,
    timestamp: Date.now(),
    original_length: text.length,
    truncated: false,
  };
}

// payload 不是合法 base64，解码时抛错
function brokenMessage() {
  return { ...message("x"), payload: "%%%%" };
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

    emitTauriEvent("mqtt-messages", [message("m1"), message("m2"), message("m3")]);
    await waitFlush();

    expect(texts(store.getServerMessages(SERVER_ID).map((m) => m.payload))).toEqual([
      "m3",
      "m2",
      "m1",
    ]);
  });

  it("单条消息处理异常后，后续批次的消息仍能进入列表", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitTauriEvent("mqtt-messages", [message("ok1"), brokenMessage()]);
    emitTauriEvent("mqtt-messages", [message("ok2")]);
    await waitFlush();

    const received = texts(store.getServerMessages(SERVER_ID).map((m) => m.payload));
    expect(received).toContain("ok2");
    expect(received).toContain("ok1");
  });

  it("快路径与慢路径共存时跨批次顺序正确", async () => {
    const store = useMqttStore();
    await store.initListeners();

    // 第一批走慢路径（脚本缓存尚未建立）
    emitTauriEvent("mqtt-messages", [message("a1"), message("a2")]);
    await waitFlush();

    // 第二批走同步快路径（缓存已确定无接收脚本）
    emitTauriEvent("mqtt-messages", [message("b1"), message("b2")]);
    await waitFlush();

    expect(texts(store.getServerMessages(SERVER_ID).map((m) => m.payload))).toEqual([
      "b2",
      "b1",
      "a2",
      "a1",
    ]);
  });
});
