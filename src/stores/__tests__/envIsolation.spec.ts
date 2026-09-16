import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { invokeMock, invokeArgsOf, resetTauriMock } from "./tauriMock";

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

import { useSubscriptionStore } from "@/stores/subscription";
import { useEnvStore } from "@/stores/env";

// 每个 Server 各有一套同名变量
const envByServer: Record<number, { name: string; value: string }[]> = {
  1: [{ name: "REGION", value: "alpha" }],
  2: [{ name: "REGION", value: "beta" }],
};

function setupInvoke() {
  invokeMock.mockImplementation(async (cmd: string, args: any) => {
    if (cmd === "list_env_variables") {
      return (envByServer[args.serverId] ?? []).map((v, i) => ({
        id: i + 1,
        server_id: args.serverId,
        name: v.name,
        value: v.value,
      }));
    }
    if (cmd === "add_subscription") {
      return {
        id: 1,
        server_id: args.serverId,
        topic: args.topic,
        qos: args.qos,
        is_active: true,
      };
    }
    return undefined;
  });
}

describe("环境变量按 Server 隔离", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    setupInvoke();
  });

  it("为不同 Server 添加订阅时各自使用本 Server 的变量值", async () => {
    const store = useSubscriptionStore();

    await store.addSubscription(1, "devices/{{REGION}}/cmd", 0);
    await store.addSubscription(2, "devices/{{REGION}}/cmd", 0);

    expect(invokeArgsOf("add_subscription").map((a) => a.topic)).toEqual([
      "devices/alpha/cmd",
      "devices/beta/cmd",
    ]);
  });

  it("EnvDrawer 已加载别的 Server 的变量时不影响订阅的替换结果", async () => {
    // 模拟用户先在管理抽屉里查看 Server 1 的变量
    await useEnvStore().loadVariables(1);

    await useSubscriptionStore().addSubscription(2, "devices/{{REGION}}/cmd", 0);

    expect(invokeArgsOf("add_subscription")[0].topic).toBe("devices/beta/cmd");
  });

  it("未定义的变量占位符保持原样", async () => {
    await useSubscriptionStore().addSubscription(1, "devices/{{UNKNOWN}}/cmd", 0);

    expect(invokeArgsOf("add_subscription")[0].topic).toBe("devices/{{UNKNOWN}}/cmd");
  });
});
