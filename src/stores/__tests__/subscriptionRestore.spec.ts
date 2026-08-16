import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { emitTauriEvent, invokeMock, invokeArgsOf, invokeCountOf, resetTauriMock } from "./tauriMock";

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
import { useServerStore } from "@/stores/server";
import { useSubscriptionStore } from "@/stores/subscription";

const ACTIVE_SERVER = 1;
const BACKGROUND_SERVER = 2;

// 后台 Server（非当前查看）也有自己的订阅
const subsByServer: Record<number, any[]> = {
  [ACTIVE_SERVER]: [{ id: 1, server_id: ACTIVE_SERVER, topic: "active/topic", qos: 0, is_active: true }],
  [BACKGROUND_SERVER]: [
    { id: 2, server_id: BACKGROUND_SERVER, topic: "bg/a", qos: 1, is_active: true },
    { id: 3, server_id: BACKGROUND_SERVER, topic: "bg/paused", qos: 0, is_active: false },
  ],
};

/** 模拟后端推送一次连接状态事件 */
function emitConnectionState(serverId: number, status: string) {
  emitTauriEvent("mqtt-connection-state", { server_id: serverId, status });
}

/** 事件处理里的订阅恢复是异步的，让出若干个微任务轮次 */
async function settle() {
  for (let i = 0; i < 10; i++) await Promise.resolve();
}

describe("断线重连后的订阅恢复", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "get_subscriptions") return subsByServer[args.serverId] ?? [];
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "list_env_variables") return [];
      return undefined;
    });
  });

  it("非活跃 Server 重连后自动恢复其全部活跃订阅", async () => {
    const serverStore = useServerStore();
    serverStore.setActiveServer(ACTIVE_SERVER);

    const store = useMqttStore();
    await store.initListeners();

    // 后台 Server 断线重连：connecting → connected
    emitConnectionState(BACKGROUND_SERVER, "connecting");
    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();

    const calls = invokeArgsOf("mqtt_subscribe");
    expect(calls).toEqual([
      { serverId: BACKGROUND_SERVER, topic: "bg/a", qos: 1 },
    ]);
  });

  it("活跃 Server 连接成功后同样恢复订阅", async () => {
    const serverStore = useServerStore();
    serverStore.setActiveServer(ACTIVE_SERVER);

    const store = useMqttStore();
    await store.initListeners();

    emitConnectionState(ACTIVE_SERVER, "connecting");
    emitConnectionState(ACTIVE_SERVER, "connected");
    await settle();

    expect(invokeArgsOf("mqtt_subscribe")).toEqual([
      { serverId: ACTIVE_SERVER, topic: "active/topic", qos: 0 },
    ]);
  });

  it("切换活跃 Server 不触发任何重订阅", async () => {
    const serverStore = useServerStore();
    const subscriptionStore = useSubscriptionStore();
    serverStore.setActiveServer(ACTIVE_SERVER);
    await subscriptionStore.fetchSubscriptions(ACTIVE_SERVER);

    const store = useMqttStore();
    await store.initListeners();

    // 两个 Server 都已连接
    emitConnectionState(ACTIVE_SERVER, "connected");
    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();
    const before = invokeCountOf("mqtt_subscribe");

    // 用户切换当前查看的 Server：不应产生任何额外订阅
    serverStore.setActiveServer(BACKGROUND_SERVER);
    await settle();
    serverStore.setActiveServer(ACTIVE_SERVER);
    await settle();

    expect(invokeCountOf("mqtt_subscribe")).toBe(before);
  });

  it("状态未发生转变的重复 connected 事件不重复订阅", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();
    const before = invokeCountOf("mqtt_subscribe");
    expect(before).toBe(1);

    emitConnectionState(BACKGROUND_SERVER, "connected");
    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();

    expect(invokeCountOf("mqtt_subscribe")).toBe(before);
  });

  it("断开后再次连上会重新恢复订阅", async () => {
    const store = useMqttStore();
    await store.initListeners();

    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();
    emitConnectionState(BACKGROUND_SERVER, "disconnected");
    emitConnectionState(BACKGROUND_SERVER, "connected");
    await settle();

    expect(invokeCountOf("mqtt_subscribe")).toBe(2);
  });
});
