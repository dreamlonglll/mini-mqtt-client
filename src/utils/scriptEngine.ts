import type { Script } from "@/stores/script";

/**
 * 脚本执行引擎
 * 安全地执行用户定义的 JavaScript 脚本
 */
export class ScriptEngine {
  /**
   * 执行发送前处理脚本
   * @param scripts 要执行的脚本列表
   * @param payload 原始 payload
   * @returns 处理后的 payload
   */
  static async executeBeforePublish(scripts: Script[], payload: string): Promise<string> {
    let result = payload;
    
    for (const script of scripts) {
      try {
        result = await this.executeScript(script.code, { payload: result });
      } catch (error) {
        console.error(`脚本执行失败 [${script.name}]:`, error);
        // 脚本执行失败时，保留原始值继续
      }
    }
    
    return result;
  }

  /**
   * 执行接收后处理脚本
   * @param scripts 要执行的脚本列表
   * @param payload 原始 payload
   * @param topic 消息主题
   * @returns 处理后的 payload
   */
  static async executeAfterReceive(scripts: Script[], payload: string, topic: string): Promise<string> {
    let result = payload;
    
    for (const script of scripts) {
      try {
        result = await this.executeScript(script.code, { payload: result, topic });
      } catch (error) {
        console.error(`脚本执行失败 [${script.name}]:`, error);
        // 脚本执行失败时，保留原始值继续
      }
    }
    
    return result;
  }

  /**
   * 执行单个脚本
   * @param code 脚本代码
   * @param context 执行上下文
   * @returns 脚本返回值
   */
  private static async executeScript(
    code: string,
    context: { payload: string; topic?: string }
  ): Promise<string> {
    // 创建沙箱环境
    const sandbox = {
      payload: context.payload,
      topic: context.topic || "",
      console: {
        log: (...args: any[]) => console.log("[Script]", ...args),
        error: (...args: any[]) => console.error("[Script]", ...args),
        warn: (...args: any[]) => console.warn("[Script]", ...args),
      },
      JSON: JSON,
      parseInt: parseInt,
      parseFloat: parseFloat,
      String: String,
      Number: Number,
      Boolean: Boolean,
      Array: Array,
      Object: Object,
      Date: Date,
      Math: Math,
      encodeURIComponent: encodeURIComponent,
      decodeURIComponent: decodeURIComponent,
      atob: atob,
      btoa: btoa,
    };

    // 包装代码，确保能够获取返回值
    // 如果代码定义了 process 函数，自动调用它
    // 使用 async 包装以支持异步函数
    const wrappedCode = `
      "use strict";
      return (async () => {
        ${code}
        // 如果定义了 process 函数，调用它并返回结果
        if (typeof process === 'function') {
          return await process(payload, topic);
        }
        return payload;
      })();
    `;

    // 创建执行函数
    const fn = new Function(
      ...Object.keys(sandbox),
      wrappedCode
    );

    // 执行并获取结果（可能是 Promise）
    let result = fn(...Object.values(sandbox));
    if (result instanceof Promise) {
      result = await result;
    }
    
    // 确保返回字符串
    if (result === undefined || result === null) {
      return context.payload;
    }
    
    if (typeof result === "string") {
      return result;
    }
    
    if (typeof result === "object") {
      return JSON.stringify(result);
    }
    
    return String(result);
  }

  /**
   * 验证脚本语法
   * @param code 脚本代码
   * @returns 错误信息或 null
   */
  static validateScript(code: string): string | null {
    try {
      new Function("payload", "topic", `"use strict"; ${code}`);
      return null;
    } catch (error: any) {
      return error.message || "脚本语法错误";
    }
  }
}
