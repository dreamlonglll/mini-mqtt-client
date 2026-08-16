/**
 * 环境变量替换工具
 *
 * 用于在 Topic 和 Payload 中替换 {{变量名}} 格式的环境变量。
 * 全项目唯一实现：发布管线、订阅（新增与编辑）、脚本沙箱的 env.replace 全部委托此处。
 */

/**
 * 替换文本中的环境变量
 *
 * 语义要点：
 * - **单遍替换**：一次扫描原文，变量值中出现的 `{{OTHER}}` 不会被再次替换；
 * - **无正则注入**：变量名只作为一次匹配的捕获结果参与查表，从不拼进正则，
 *   因此不需要（也不能）对变量名做转义——含正则元字符的变量名既不会抛错，
 *   也不会误命中其他变量；
 * - 用替换函数而非替换字符串，变量值中的 `$&`、`$1` 按字面量处理。
 *
 * @param text 原始文本
 * @param variables 变量映射 { name: value }
 * @returns 替换后的文本
 */
export function replaceEnvVariables(
  text: string,
  variables: Record<string, string>
): string {
  if (!text) return text;
  return text.replace(/\{\{(\w+)\}\}/g, (match, varName) => {
    return variables[varName] ?? match;
  });
}

/**
 * 检查文本中是否包含环境变量占位符
 * @param text 要检查的文本
 * @returns 是否包含环境变量
 */
export function hasEnvVariables(text: string): boolean {
  if (!text) return false;
  return /\{\{\w+\}\}/.test(text);
}

/**
 * 提取文本中的所有环境变量名
 * @param text 要检查的文本
 * @returns 变量名数组
 */
export function extractEnvVariableNames(text: string): string[] {
  if (!text) return [];
  const matches = text.matchAll(/\{\{(\w+)\}\}/g);
  const names = new Set<string>();
  for (const match of matches) {
    names.add(match[1]);
  }
  return Array.from(names);
}

/**
 * 检查是否有未定义的环境变量
 * @param text 要检查的文本
 * @param variables 已定义的变量映射
 * @returns 未定义的变量名数组
 */
export function getUndefinedVariables(
  text: string,
  variables: Record<string, string>
): string[] {
  const names = extractEnvVariableNames(text);
  return names.filter((name) => !(name in variables));
}
