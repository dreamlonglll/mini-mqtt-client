import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { invokeMock, invokeCountOf, resetTauriMock } from "./tauriMock";

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
import { useScriptStore } from "@/stores/script";
import { useEnvStore } from "@/stores/env";

const SERVER_ID = 1;

describe("脚本缓存失效", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_enabled_scripts" || cmd === "list_scripts") return [];
      if (cmd === "create_script") return 1;
      return undefined;
    });
  });

  // 先预热缓存，再执行 CRUD，最后断言下一次取脚本重新走了 IPC
  async function expectCacheInvalidatedBy(action: () => Promise<void>) {
    const mqttStore = useMqttStore();
    await mqttStore.getCachedScripts(SERVER_ID, "before_publish");
    await action();
    const before = invokeCountOf("get_enabled_scripts");
    await mqttStore.getCachedScripts(SERVER_ID, "before_publish");
    expect(invokeCountOf("get_enabled_scripts")).toBe(before + 1);
  }

  it("创建脚本后缓存立即失效", async () => {
    const scriptStore = useScriptStore();
    await expectCacheInvalidatedBy(async () => {
      await scriptStore.createScript({
        server_id: SERVER_ID,
        name: "s",
        script_type: "before_publish",
        code: "",
        enabled: true,
      });
    });
  });

  it("更新脚本后缓存立即失效", async () => {
    const scriptStore = useScriptStore();
    await expectCacheInvalidatedBy(async () => {
      await scriptStore.updateScript({ id: 1, code: "x" }, SERVER_ID);
    });
  });

  it("删除脚本后缓存立即失效", async () => {
    const scriptStore = useScriptStore();
    await expectCacheInvalidatedBy(async () => {
      await scriptStore.deleteScript(1, SERVER_ID);
    });
  });

  it("启停脚本后缓存立即失效", async () => {
    const scriptStore = useScriptStore();
    await expectCacheInvalidatedBy(async () => {
      await scriptStore.toggleScript(1, false, SERVER_ID);
    });
  });
});

describe("环境变量缓存失效", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "list_env_variables") {
        return [{ id: 1, server_id: args.serverId, name: "REGION", value: "alpha" }];
      }
      if (cmd === "create_env_variable") return 2;
      return undefined;
    });
  });

  async function expectCacheInvalidatedBy(action: () => Promise<void>) {
    const mqttStore = useMqttStore();
    await mqttStore.getCachedEnvVariables(SERVER_ID);
    await action();
    const before = invokeCountOf("list_env_variables");
    await mqttStore.getCachedEnvVariables(SERVER_ID);
    expect(invokeCountOf("list_env_variables")).toBe(before + 1);
  }

  it("创建环境变量后缓存立即失效", async () => {
    const envStore = useEnvStore();
    await expectCacheInvalidatedBy(async () => {
      await envStore.createVariable({ server_id: SERVER_ID, name: "PORT", value: "1883" });
    });
  });

  it("更新环境变量后缓存立即失效", async () => {
    const envStore = useEnvStore();
    await envStore.loadVariables(SERVER_ID);
    await expectCacheInvalidatedBy(async () => {
      await envStore.updateVariable({ id: 1, value: "beta" });
    });
  });

  it("删除环境变量后缓存立即失效", async () => {
    const envStore = useEnvStore();
    await envStore.loadVariables(SERVER_ID);
    await expectCacheInvalidatedBy(async () => {
      await envStore.deleteVariable(1);
    });
  });
});
