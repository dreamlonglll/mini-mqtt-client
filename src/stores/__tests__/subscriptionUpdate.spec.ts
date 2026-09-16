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

const envByServer: Record<number, { name: string; value: string }[]> = {
  1: [{ name: "REGION", value: "alpha" }],
  2: [{ name: "REGION", value: "beta" }],
};

describe("编辑订阅时的变量替换", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "list_env_variables") {
        return (envByServer[args.serverId] ?? []).map((v, i) => ({
          id: i + 1,
          server_id: args.serverId,
          name: v.name,
          value: v.value,
        }));
      }
      if (cmd === "update_subscription") {
        return {
          id: args.request.id,
          server_id: args.serverId,
          topic: args.request.topic ?? args.oldTopic,
          qos: args.request.qos ?? 0,
          is_active: true,
        };
      }
      return undefined;
    });
  });

  it("更新订阅时 topic 中的变量按本 Server 的值替换后入库", async () => {
    const store = useSubscriptionStore();

    await store.updateSubscription(1, "old/topic", {
      id: 7,
      topic: "devices/{{REGION}}/cmd",
      qos: 1,
    });

    expect(invokeArgsOf("update_subscription")[0].request.topic).toBe("devices/alpha/cmd");
  });

  it("不同 Server 的同名变量各自取自己的值", async () => {
    const store = useSubscriptionStore();

    await store.updateSubscription(2, "old/topic", {
      id: 8,
      topic: "devices/{{REGION}}/cmd",
    });

    expect(invokeArgsOf("update_subscription")[0].request.topic).toBe("devices/beta/cmd");
  });

  it("只改颜色（不带 topic）时不触发替换也不报错", async () => {
    const store = useSubscriptionStore();

    await store.updateSubscription(1, "devices/alpha/cmd", {
      id: 9,
      color: "#409EFF",
    });

    expect(invokeArgsOf("update_subscription")[0].request.topic).toBeUndefined();
  });
});
