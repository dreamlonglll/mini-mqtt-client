/**
 * 环境变量替换工具
 * 
 * 用于在 Topic 和 Payload 中替换 {{变量名}} 格式的环境变量
 */

/**
 * 替换文本中的环境变量
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
