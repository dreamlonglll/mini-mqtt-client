import { describe, it, expect } from "vitest";
import { replaceEnvVariables } from "@/utils/envReplacer";

describe("环境变量替换（唯一实现）", () => {
  it("单遍替换：变量值里出现的占位符不会被二次替换", () => {
    const variables = { A: "{{B}}", B: "真实值" };
    expect(replaceEnvVariables("{{A}}", variables)).toBe("{{B}}");
  });

  it("同一个变量出现多次全部被替换", () => {
    expect(replaceEnvVariables("{{X}}/{{X}}", { X: "1" })).toBe("1/1");
  });

  it("未定义的变量占位符保持原样", () => {
    expect(replaceEnvVariables("a/{{NOPE}}/b", { X: "1" })).toBe("a/{{NOPE}}/b");
  });

  it("含正则特殊字符的变量名不抛异常且不会误命中其他变量", () => {
    // 若实现为"逐变量 new RegExp 且不转义"，"A.C" 会把 {{ABC}} 误替换成 x
    const variables = { "A.C": "x", "A+B": "y", ABC: "命中" };
    expect(() => replaceEnvVariables("{{A.C}} {{A+B}}", variables)).not.toThrow();
    expect(replaceEnvVariables("{{ABC}}", variables)).toBe("命中");
  });

  it("变量值中的 $& 等替换模式按字面量处理", () => {
    expect(replaceEnvVariables("{{V}}", { V: "$&$1$$" })).toBe("$&$1$$");
  });

  it("空文本原样返回", () => {
    expect(replaceEnvVariables("", { X: "1" })).toBe("");
  });
});
