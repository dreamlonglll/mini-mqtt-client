import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { invokeMock, invokeArgsOf, invokeCountOf, resetTauriMock } from "./tauriMock";

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

import { usePublishPipeline } from "@/composables/usePublishPipeline";
import { useMqttStore } from "@/stores/mqtt";

const SERVER_ID = 1;

/** before_publish 脚本表（按用例改写） */
let scripts: any[] = [];

function setupInvoke() {
  invokeMock.mockImplementation(async (cmd: string, args: any) => {
    if (cmd === "list_env_variables") {
      return [{ id: 1, server_id: args.serverId, name: "REGION", value: "alpha" }];
    }
    if (cmd === "get_enabled_scripts") {
      return args.scriptType === "before_publish" ? scripts : [];
    }
    if (cmd === "publish_message") {
      return { id: 1, server_id: args.serverId, ...args.message };
    }
    return undefined;
  });
}

describe("统一发布管线", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    resetTauriMock();
    scripts = [];
    setupInvoke();
  });

  it("TEXT 消息按参数发往 publish_message", async () => {
    const { publish } = usePublishPipeline();

    const result = await publish({
      serverId: SERVER_ID,
      topic: "dev/{{REGION}}/cmd",
      payload: "hello {{REGION}}",
      qos: 1,
      retain: true,
      format: "text",
    });

    expect(result.success).toBe(true);
    expect(invokeArgsOf("publish_message")).toEqual([
      {
        serverId: SERVER_ID,
        message: {
          topic: "dev/alpha/cmd",
          payload: "hello alpha",
          qos: 1,
          retain: true,
          format: "text",
        },
      },
    ]);
  });

  it("HEX 消息以原始十六进制字符串加 format=hex 交给 Rust 解码", async () => {
    const { publish } = usePublishPipeline();

    const result = await publish({
      serverId: SERVER_ID,
      topic: "dev/hex",
      payload: "01 0A FF",
      qos: 0,
      retain: false,
      format: "hex",
    });

    expect(result.success).toBe(true);
    expect(invokeArgsOf("publish_message")[0].message).toEqual({
      topic: "dev/hex",
      payload: "01 0A FF",
      qos: 0,
      retain: false,
      format: "hex",
    });
  });

  it("HEX 消息入队 UI 时携带 payload 类型并按字节解码", async () => {
    const mqttStore = useMqttStore();
    const { publish } = usePublishPipeline();

    await publish({
      serverId: SERVER_ID,
      topic: "dev/hex",
      payload: "010AFF",
      qos: 0,
      retain: false,
      format: "hex",
    });
    // 消息队列有 50ms 攒批，等待其 flush
    await new Promise((r) => setTimeout(r, 80));

    const messages = mqttStore.getServerMessages(SERVER_ID);
    expect(messages).toHaveLength(1);
    expect(messages[0].payload_type).toBe("hex");
    expect(Array.from(messages[0].payload!)).toEqual([0x01, 0x0a, 0xff]);
  });

  it("before_publish 脚本抛错时不发布，结果为失败并带脚本错误", async () => {
    scripts = [
      {
        id: 1,
        server_id: SERVER_ID,
        name: "炸裂脚本",
        script_type: "before_publish",
        code: "async function process(p) { throw new Error('boom'); }",
        enabled: true,
      },
    ];

    const mqttStore = useMqttStore();
    const { publish } = usePublishPipeline();

    const result = await publish({
      serverId: SERVER_ID,
      topic: "dev/{{REGION}}/cmd",
      payload: "原文",
      qos: 0,
      retain: false,
      format: "text",
    });

    expect(result.success).toBe(false);
    expect(result.scriptError).toContain("boom");
    expect(invokeCountOf("publish_message")).toBe(0);

    await new Promise((r) => setTimeout(r, 80));
    const messages = mqttStore.getServerMessages(SERVER_ID);
    expect(messages).toHaveLength(1);
    expect(messages[0].scriptError).toContain("boom");
  });

  it("变量替换发生在脚本执行之前（脚本看到的是替换后的内容）", async () => {
    scripts = [
      {
        id: 1,
        server_id: SERVER_ID,
        name: "回显脚本",
        script_type: "before_publish",
        code: "async function process(p, t) { return p + '|' + t; }",
        enabled: true,
      },
    ];

    const { publish } = usePublishPipeline();
    await publish({
      serverId: SERVER_ID,
      topic: "dev/{{REGION}}",
      payload: "{{REGION}}",
      qos: 0,
      retain: false,
      format: "text",
    });

    expect(invokeArgsOf("publish_message")[0].message.payload).toBe("alpha|dev/alpha");
  });

  it("后端发布失败时结果为失败且携带错误信息", async () => {
    invokeMock.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === "list_env_variables") return [];
      if (cmd === "get_enabled_scripts") return [];
      if (cmd === "publish_message") throw new Error("未连接");
      return undefined;
    });

    const { publish } = usePublishPipeline();
    const result = await publish({
      serverId: SERVER_ID,
      topic: "dev/x",
      payload: "p",
      qos: 0,
      retain: false,
      format: "text",
    });

    expect(result.success).toBe(false);
    expect(result.scriptError).toBeUndefined();
    expect(result.error).toContain("未连接");
  });
});
