import { describe, it, expect } from "vitest";
import { localeMessages, supportedLocales } from "@/i18n";

/**
 * 收集消息对象的全部叶子键路径
 *
 * 语言 YAML 经 @intlify/unplugin-vue-i18n 预编译后，叶子是消息函数而非字符串，
 * 因此以"不再是普通对象"作为叶子判据。
 */
function collectKeys(node: unknown, prefix = "", out: string[] = []): string[] {
  if (node === null || typeof node !== "object" || Array.isArray(node)) {
    out.push(prefix);
    return out;
  }
  for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
    collectKeys(value, prefix ? `${prefix}.${key}` : key, out);
  }
  return out;
}

describe("i18n 语言文件键集合对齐", () => {
  it("zh-CN 与 en-US 的键集合完全一致", () => {
    const zh = new Set(collectKeys(localeMessages["zh-CN"]));
    const en = new Set(collectKeys(localeMessages["en-US"]));

    const onlyZh = [...zh].filter((k) => !en.has(k)).sort();
    const onlyEn = [...en].filter((k) => !zh.has(k)).sort();

    expect({ onlyZh, onlyEn }).toEqual({ onlyZh: [], onlyEn: [] });
  });

  it("两个语言文件都非空且键数量一致", () => {
    const zh = collectKeys(localeMessages["zh-CN"]);
    const en = collectKeys(localeMessages["en-US"]);

    expect(zh.length).toBeGreaterThan(0);
    expect(zh.length).toBe(en.length);
  });

  it("supportedLocales 中的每种语言都有消息定义", () => {
    for (const locale of supportedLocales) {
      expect(collectKeys(localeMessages[locale]).length).toBeGreaterThan(0);
    }
  });

  it("不存在只有子节点没有叶子的空分组（避免渲染出原始键名）", () => {
    for (const locale of supportedLocales) {
      const emptyGroups: string[] = [];
      const walk = (node: unknown, prefix: string) => {
        if (node === null || typeof node !== "object" || Array.isArray(node)) return;
        const entries = Object.entries(node as Record<string, unknown>);
        if (entries.length === 0) {
          emptyGroups.push(prefix);
          return;
        }
        for (const [key, value] of entries) {
          walk(value, prefix ? `${prefix}.${key}` : key);
        }
      };
      walk(localeMessages[locale], "");
      expect(emptyGroups).toEqual([]);
    }
  });
});
