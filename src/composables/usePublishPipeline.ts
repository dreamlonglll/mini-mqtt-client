import { useMqttStore } from "@/stores/mqtt";
import { useMessageStore } from "@/stores/message";
import { ScriptEngine } from "@/utils/scriptEngine";
import { replaceEnvVariables } from "@/utils/envReplacer";
import { handleScriptError } from "@/utils/errorHandler";

/** 发布 payload 的格式类型 */
export type PublishFormat = "json" | "hex" | "text";

/** 一次发布请求（调用方传入的原始内容，变量尚未替换） */
export interface PublishRequest {
  serverId: number;
  topic: string;
  payload: string;
  qos: 0 | 1 | 2;
  retain: boolean;
  format: PublishFormat;
}

/** 发布结果：调用方据此更新自己的 UI（提示、计数、日志） */
export interface PublishResult {
  success: boolean;
  /** 实际使用的 topic（已完成变量替换） */
  topic: string;
  /** 实际发出的 payload（已替换变量并经 before_publish 脚本处理）；失败时为未处理内容 */
  payload: string;
  /** 失败原因（脚本失败或后端发布失败） */
  error?: string;
  /** 失败来自 before_publish 脚本时的错误信息 */
  scriptError?: string;
}

/**
 * 统一发布管线
 *
 * 手动发布面板与定时发布对话框共用这一条管线，固定五步：
 * 1. 按 Server 取环境变量并替换 topic / payload；
 * 2. 执行 before_publish 脚本，失败即中止本条发送（未处理的原文不能发出）；
 * 3. 调用 `publish_message` 按 payload 类型发布（HEX 由 Rust 侧解码）；
 * 4. 发布历史由 `publish_message` 命令内部写入，前端不再重复落库；
 * 5. 携带 payload 类型入队 UI 消息列表。
 *
 * 管线只负责发布语义与脚本错误上报，成功 / 失败的界面提示交给调用方。
 */
export function usePublishPipeline() {
  const mqttStore = useMqttStore();
  const messageStore = useMessageStore();

  async function publish(request: PublishRequest): Promise<PublishResult> {
    const { serverId, qos, retain, format } = request;

    // 1. 环境变量替换（按 Server 隔离的缓存，切换 Server 不会串用别人的变量）
    const envVariables = await mqttStore.getCachedEnvVariables(serverId);
    const topic = replaceEnvVariables(request.topic, envVariables);
    let payload = replaceEnvVariables(request.payload, envVariables);

    // 2. 发送前处理脚本（复用 mqttStore 的脚本缓存，高频发布不必每条走 IPC）
    try {
      const scripts = await mqttStore.getCachedScripts(serverId, "before_publish");
      if (scripts.length > 0) {
        payload = await ScriptEngine.executeBeforePublish(
          scripts,
          payload,
          topic,
          envVariables
        );
      }
    } catch (error: any) {
      const scriptError = error?.message || String(error);
      handleScriptError(error);

      // 原始消息带错误标记入队（不实际发布），便于在消息列表中定位失败的那一条
      mqttStore.addPublishMessage(serverId, {
        topic,
        payload: request.payload,
        qos,
        retain,
        scriptError,
        payload_type: format,
      });

      return { success: false, topic, payload: request.payload, error: scriptError, scriptError };
    }

    // 3 + 4. 交给后端发布并写入发布历史（HEX 在 Rust 侧解码）
    try {
      await messageStore.publishMessage(serverId, {
        topic,
        payload,
        qos,
        retain,
        format,
      });
    } catch (error: any) {
      return { success: false, topic, payload, error: error?.message || String(error) };
    }

    // 5. UI 消息列表入队，携带 payload 类型（HEX 不会被当作文本展示）
    mqttStore.addPublishMessage(serverId, {
      topic,
      payload,
      qos,
      retain,
      payload_type: format,
    });

    return { success: true, topic, payload };
  }

  return { publish };
}
