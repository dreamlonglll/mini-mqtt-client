import { describe, it, expect, beforeEach, vi } from "vitest";

// errorHandler 会走 element-plus 通知与 invoke 写日志，这里只关心脚本引擎本身
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("element-plus", () => ({
  ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
  ElNotification: vi.fn(),
}));

import { ScriptEngine } from "@/utils/scriptEngine";
import { replaceEnvVariables } from "@/utils/envReplacer";
import type { Script } from "@/stores/script";

function makeScript(code: string, name = "s"): Script {
  return { id: 1, server_id: 1, name, script_type: "before_publish", code, enabled: true };
}

describe("脚本沙箱的 env.replace", () => {
  it("与 envReplacer 结果一致（单遍、不链式二次替换）", async () => {
    const variables = { A: "{{B}}", B: "真实值" };
    const out = await ScriptEngine.executeBeforePublish(
      [makeScript("async function process(p) { return env.replace(p); }")],
      "{{A}}",
      "t/topic",
      variables
    );
    expect(out).toBe(replaceEnvVariables("{{A}}", variables));
    expect(out).toBe("{{B}}");
  });

  it("含正则特殊字符的变量名不会误命中其他变量", async () => {
    const variables = { "A.C": "x", ABC: "命中" };
    const out = await ScriptEngine.executeBeforePublish(
      [makeScript("async function process(p) { return env.replace(p); }")],
      "{{ABC}}",
      "t/topic",
      variables
    );
    expect(out).toBe(replaceEnvVariables("{{ABC}}", variables));
    expect(out).toBe("命中");
  });
});

describe("脚本编译缓存的淘汰策略", () => {
  beforeEach(() => {
    ScriptEngine.clearCompileCache();
  });

  it("缓存满后只逐出最旧一条，其余仍然命中", async () => {
    const code = (i: number) => `async function process(p) { return p + "${i}"; }`;

    for (let i = 0; i < 100; i++) {
      await ScriptEngine.executeBeforePublish([makeScript(code(i))], "x", "t");
    }
    expect(ScriptEngine.compileCacheSize).toBe(100);

    // 第 101 段脚本入缓存
    await ScriptEngine.executeBeforePublish([makeScript(code(100))], "x", "t");

    expect(ScriptEngine.compileCacheSize).toBe(100);
    expect(ScriptEngine.isCompiled(code(0))).toBe(false);
    expect(ScriptEngine.isCompiled(code(1))).toBe(true);
    expect(ScriptEngine.isCompiled(code(99))).toBe(true);
    expect(ScriptEngine.isCompiled(code(100))).toBe(true);
  });
});

describe("脚本语法校验", () => {
  it("含顶层 await 的合法脚本通过校验", () => {
    expect(ScriptEngine.validateScript("const v = await Promise.resolve(1);")).toBeNull();
  });

  it("含 async process 的常规脚本通过校验", () => {
    expect(
      ScriptEngine.validateScript("async function process(payload) { return payload; }")
    ).toBeNull();
  });

  it("真语法错误被拒绝且返回非空错误信息", () => {
    const error = ScriptEngine.validateScript("function process( { return");
    expect(error).toBeTruthy();
    expect(String(error).length).toBeGreaterThan(0);
  });
});
