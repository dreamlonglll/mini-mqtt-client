import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import {
  emitMessageBatch,
  emitTauriEvent,
  invokeCountOf,
  invokeMock,
  listenMock,
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

const SERVER = 1;

/** 让攒批队列跑完 */
async function settle() {
  for (let i = 0; i < 10; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 80));
}

function receivedMessage(payload: string): ReceivedMessage {
  return {
    server_id: SERVER,
    topic: "t/a",
    payload: new TextEncoder().encode(payload),
    qos: 0,
    retain: false,
    timestamp: Date.now(),
    original_length: payload.length,
    truncated: false,
  };
}

describe("initListeners 幂等", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "list_env_variables") return [];
      if (cmd === "get_subscriptions") return [];
      return undefined;
    });
  });

  it("重复调用后一批消息只入队一次", async () => {
    const store = useMqttStore();
    await store.initListeners();
    await store.initListeners();
    await store.initListeners();

    emitMessageBatch([receivedMessage("hello")]);
    await settle();

    expect(store.getServerMessages(SERVER).length).toBe(1);
  });

  it("重复调用不会重复注册 listen，也只注册一个消息 Channel", async () => {
    const store = useMqttStore();
    await store.initListeners();
    const afterFirst = listenMock.mock.calls.length;

    await store.initListeners();
    await store.initListeners();

    expect(listenMock.mock.calls.length).toBe(afterFirst);
    expect(invokeCountOf("register_message_channel")).toBe(1);
  });

  it("并发调用同样只注册一份监听", async () => {
    const store = useMqttStore();
    await Promise.all([
      store.initListeners(),
      store.initListeners(),
      store.initListeners(),
    ]);

    expect(invokeCountOf("register_message_channel")).toBe(1);
    emitMessageBatch([receivedMessage("hi")]);
    await settle();

    expect(store.getServerMessages(SERVER).length).toBe(1);
  });

  it("disposeListeners 后可重新初始化，旧 Channel 静默、新 Channel 生效", async () => {
    const store = useMqttStore();
    await store.initListeners();
    store.disposeListeners();
    await store.initListeners();

    expect(invokeCountOf("register_message_channel")).toBe(2);
    emitMessageBatch([receivedMessage("again")]);
    await settle();

    expect(store.getServerMessages(SERVER).length).toBe(1);
  });

  it("重复调用不会重复处理连接状态事件（订阅只恢复一次）", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_subscriptions")
        return [{ id: 1, server_id: SERVER, topic: "t/a", qos: 0, is_active: true }];
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "list_env_variables") return [];
      return undefined;
    });

    const store = useMqttStore();
    await store.initListeners();
    await store.initListeners();

    emitTauriEvent("mqtt-connection-state", { server_id: SERVER, status: "connected" });
    await settle();

    expect(
      invokeMock.mock.calls.filter(([cmd]) => cmd === "mqtt_subscribe").length
    ).toBe(1);
  });
});
