import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { emitTauriEvent, invokeMock, resetTauriMock } from "./tauriMock";

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
import { useServerStore } from "@/stores/server";
import { useSubscriptionStore } from "@/stores/subscription";

const DOOMED = 1;
const KEPT = 2;

/** 让攒批队列与串行链跑完 */
async function settle() {
  for (let i = 0; i < 10; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 80));
}

describe("删除 Server 后的级联清理", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "get_servers") {
        return [
          { id: DOOMED, name: "doomed", host: "h", port: 1883 },
          { id: KEPT, name: "kept", host: "h", port: 1883 },
        ];
      }
      if (cmd === "get_subscriptions") {
        return [{ id: 10, server_id: args.serverId, topic: "t", qos: 0, is_active: true }];
      }
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "list_env_variables") return [];
      return undefined;
    });
  });

  it("删除 Server 后 mqttStore 与 subscriptionStore 都不再残留其状态", async () => {
    const serverStore = useServerStore();
    const mqttStore = useMqttStore();
    const subscriptionStore = useSubscriptionStore();

    await serverStore.fetchServers();
    await mqttStore.initListeners();

    // 制造两个 Server 的残留状态：连接状态、消息、订阅缓存
    emitTauriEvent("mqtt-connection-state", { server_id: DOOMED, status: "connected" });
    emitTauriEvent("mqtt-connection-state", { server_id: KEPT, status: "connected" });
    await subscriptionStore.fetchSubscriptions(DOOMED);
    await subscriptionStore.fetchSubscriptions(KEPT);
    mqttStore.addPublishMessage(DOOMED, {
      topic: "a",
      payload: "hello",
      qos: 0,
      retain: false,
    });
    mqttStore.addPublishMessage(KEPT, {
      topic: "b",
      payload: "world",
      qos: 0,
      retain: false,
    });
    await settle();

    expect(mqttStore.connectionStates.has(DOOMED)).toBe(true);
    expect(mqttStore.getServerMessages(DOOMED).length).toBe(1);
    expect(subscriptionStore.subscriptions.has(DOOMED)).toBe(true);

    await serverStore.removeServer(DOOMED);
    await settle();

    // 被删 Server 的残留全部清干净
    expect(mqttStore.connectionStates.has(DOOMED)).toBe(false);
    expect(mqttStore.getServerMessages(DOOMED)).toEqual([]);
    expect(mqttStore.messagesByServer.has(DOOMED)).toBe(false);
    expect(subscriptionStore.subscriptions.has(DOOMED)).toBe(false);
    expect(serverStore.servers.some((s) => s.server.id === DOOMED)).toBe(false);

    // 其他 Server 的状态不受影响
    expect(mqttStore.connectionStates.has(KEPT)).toBe(true);
    expect(mqttStore.getServerMessages(KEPT).length).toBe(1);
    expect(subscriptionStore.subscriptions.has(KEPT)).toBe(true);
  });

  it("删除 Server 后该 Server 的脚本与环境变量缓存被失效", async () => {
    const serverStore = useServerStore();
    const mqttStore = useMqttStore();
    await serverStore.fetchServers();

    // 预热缓存
    await mqttStore.getCachedScripts(DOOMED, "after_receive");
    await mqttStore.getCachedEnvVariables(DOOMED);
    const before = invokeMock.mock.calls.filter(
      ([cmd]) => cmd === "get_enabled_scripts" || cmd === "list_env_variables"
    ).length;
    expect(before).toBe(2);

    // 缓存命中：不产生新的 invoke
    await mqttStore.getCachedScripts(DOOMED, "after_receive");
    await mqttStore.getCachedEnvVariables(DOOMED);
    expect(
      invokeMock.mock.calls.filter(
        ([cmd]) => cmd === "get_enabled_scripts" || cmd === "list_env_variables"
      ).length
    ).toBe(before);

    await serverStore.removeServer(DOOMED);

    // 删除后缓存失效：再取会重新走后端
    await mqttStore.getCachedScripts(DOOMED, "after_receive");
    await mqttStore.getCachedEnvVariables(DOOMED);
    expect(
      invokeMock.mock.calls.filter(
        ([cmd]) => cmd === "get_enabled_scripts" || cmd === "list_env_variables"
      ).length
    ).toBe(before + 2);
  });
});
