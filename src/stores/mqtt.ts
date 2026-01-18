import { defineStore } from "pinia";
import { ref, shallowRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import type { ConnectionStatus, MqttMessage, EnvVariable } from "@/types/mqtt";
import { ScriptEngine } from "@/utils/scriptEngine";
import type { Script } from "@/stores/script";
import { handleScriptError } from "@/utils/errorHandler";
import i18n from "@/i18n";

interface ConnectionState {
  server_id: number;
  status: ConnectionStatus;
  error?: string;
}

interface ReceivedMessage {
  server_id: number;
  topic: string;
  payload: number[];
  qos: number;
  retain: boolean;
  timestamp: string;
}

  // 脚本缓存接口
interface ScriptCache {
  scripts: Script[];
  timestamp: number;
}

// 环境变量缓存接口
interface EnvCache {
  variables: Record<string, string>;
  timestamp: number;
}

// 脚本缓存有效期（毫秒）
const SCRIPT_CACHE_TTL = 5000;

export const useMqttStore = defineStore("mqtt", () => {
  // 连接状态
  const connectionStates = ref<
    Map<number, { status: ConnectionStatus; error?: string }>
  >(new Map());

  // 按 serverId 分组存储消息（使用 shallowRef 减少深度响应式开销）
  const messagesByServer = shallowRef<Map<number, MqttMessage[]>>(new Map());

  // 订阅列表（按 server_id 分组）
  const subscriptions = ref<Map<number, Set<string>>>(new Map());

  // 脚本缓存（避免高频调用 invoke）
  const scriptCache = new Map<string, ScriptCache>();

  // 环境变量缓存
  const envCache = new Map<number, EnvCache>();

  // 消息批处理队列
  const messageQueue: MqttMessage[] = [];
  let batchTimeout: ReturnType<typeof setTimeout> | null = null;
  const BATCH_INTERVAL = 50; // 批处理间隔（毫秒）

  // 获取缓存的脚本
  async function getCachedScripts(serverId: number, scriptType: string): Promise<Script[]> {
    const cacheKey = `${serverId}-${scriptType}`;
    const cached = scriptCache.get(cacheKey);
    const now = Date.now();

    if (cached && now - cached.timestamp < SCRIPT_CACHE_TTL) {
      return cached.scripts;
    }

    try {
      const scripts = await invoke<Script[]>("get_enabled_scripts", {
        serverId,
        scriptType,
      });
      scriptCache.set(cacheKey, { scripts, timestamp: now });
      return scripts;
    } catch {
      return [];
    }
  }

  // 清除脚本缓存（当脚本更新时调用）
  function clearScriptCache(serverId?: number) {
    if (serverId) {
      scriptCache.delete(`${serverId}-before_send`);
      scriptCache.delete(`${serverId}-after_receive`);
    } else {
      scriptCache.clear();
    }
  }

  // 获取缓存的环境变量
  async function getCachedEnvVariables(serverId: number): Promise<Record<string, string>> {
    const cached = envCache.get(serverId);
    const now = Date.now();

    if (cached && now - cached.timestamp < SCRIPT_CACHE_TTL) {
      return cached.variables;
    }

    try {
      const envList = await invoke<EnvVariable[]>("list_env_variables", { serverId });
      const variables: Record<string, string> = {};
      for (const env of envList) {
        variables[env.name] = env.value;
      }
      envCache.set(serverId, { variables, timestamp: now });
      return variables;
    } catch {
      return {};
    }
  }

  // 清除环境变量缓存
  function clearEnvCache(serverId?: number) {
    if (serverId) {
      envCache.delete(serverId);
    } else {
      envCache.clear();
    }
  }

  // 批量处理消息队列
  function flushMessageQueue() {
    if (messageQueue.length === 0) return;

    const newMap = new Map(messagesByServer.value);
    
    // 按 serverId 分组处理
    const messagesByServerId = new Map<number, MqttMessage[]>();
    for (const msg of messageQueue) {
      if (!messagesByServerId.has(msg.server_id)) {
        messagesByServerId.set(msg.server_id, []);
      }
      messagesByServerId.get(msg.server_id)!.push(msg);
    }

    // 合并到现有消息
    for (const [serverId, newMessages] of messagesByServerId) {
      const existing = newMap.get(serverId) || [];
      const merged = [...newMessages, ...existing];
      // 限制每个 server 的消息数量
      newMap.set(serverId, merged.length > 1000 ? merged.slice(0, 1000) : merged);
    }

    messagesByServer.value = newMap;
    messageQueue.length = 0;
    batchTimeout = null;
  }

  // 添加消息到队列
  function queueMessage(msg: MqttMessage) {
    messageQueue.push(msg);
    
    if (!batchTimeout) {
      batchTimeout = setTimeout(flushMessageQueue, BATCH_INTERVAL);
    }
  }

  // 初始化事件监听
  const initListeners = async () => {
    // 监听连接状态变化
    await listen<ConnectionState>("mqtt-connection-state", (event) => {
      const { server_id, status, error } = event.payload;
      connectionStates.value.set(server_id, {
        status: status as ConnectionStatus,
        error,
      });
      
      // 如果有错误，使用 ElMessage 显示
      if (error && status === "error") {
        ElMessage.error({
          message: `${i18n.global.t('errors.connectFailed')}: ${error}`,
          duration: 5000,
        });
      }
    });

    // 监听接收消息
    await listen<ReceivedMessage>("mqtt-message", async (event) => {
      const msg = event.payload;
      let payloadBytes = new Uint8Array(msg.payload);
      let scriptError: string | undefined = undefined;
      
      // 尝试应用接收后处理脚本（使用缓存）
      try {
        const scripts = await getCachedScripts(msg.server_id, "after_receive");
        
        if (scripts.length > 0) {
          const originalPayload = new TextDecoder().decode(payloadBytes);
          const envVariables = await getCachedEnvVariables(msg.server_id);
          const processedPayload = await ScriptEngine.executeAfterReceive(
            scripts,
            originalPayload,
            msg.topic,
            envVariables
          );
          payloadBytes = new TextEncoder().encode(processedPayload);
        }
      } catch (error: any) {
        // 记录脚本错误
        scriptError = error?.message || String(error);
        handleScriptError(error, true); // 静默处理，不显示通知（会写入日志）
      }
      
      // 使用批处理队列
      queueMessage({
        server_id: msg.server_id,
        direction: "receive",
        topic: msg.topic,
        payload: payloadBytes,
        qos: msg.qos as 0 | 1 | 2,
        retain: msg.retain,
        timestamp: msg.timestamp,
        scriptError: scriptError,
      });
    });
  };

  // 连接
  const connect = async (serverId: number) => {
    connectionStates.value.set(serverId, {
      status: "connecting",
      error: undefined,
    });
    await invoke("mqtt_connect", { serverId });
  };

  // 断开连接
  const disconnect = async (serverId: number) => {
    await invoke("mqtt_disconnect", { serverId });
  };

  // 发布消息
  const publish = async (
    serverId: number,
    topic: string,
    payload: string | Uint8Array,
    qos: 0 | 1 | 2 = 0,
    retain: boolean = false
  ) => {
    const payloadBytes =
      typeof payload === "string"
        ? Array.from(new TextEncoder().encode(payload))
        : Array.from(payload);

    await invoke("mqtt_publish", {
      serverId,
      topic,
      payload: payloadBytes,
      qos,
      retain,
    });

    // 添加到消息列表（使用批处理）
    queueMessage({
      server_id: serverId,
      direction: "publish",
      topic,
      payload:
        typeof payload === "string" ? new TextEncoder().encode(payload) : payload,
      qos,
      retain,
      timestamp: new Date().toISOString(),
    });
  };

  // 订阅
  const subscribe = async (
    serverId: number,
    topic: string,
    qos: 0 | 1 | 2 = 0
  ) => {
    await invoke("mqtt_subscribe", { serverId, topic, qos });

    if (!subscriptions.value.has(serverId)) {
      subscriptions.value.set(serverId, new Set());
    }
    subscriptions.value.get(serverId)!.add(topic);
  };

  // 取消订阅
  const unsubscribe = async (serverId: number, topic: string) => {
    await invoke("mqtt_unsubscribe", { serverId, topic });
    subscriptions.value.get(serverId)?.delete(topic);
  };

  // 获取连接状态
  const getConnectionStatus = (serverId: number): ConnectionStatus => {
    return connectionStates.value.get(serverId)?.status || "disconnected";
  };

  // 获取连接错误
  const getConnectionError = (serverId: number): string | undefined => {
    return connectionStates.value.get(serverId)?.error;
  };

  // 获取某个 server 的消息（直接返回，无需过滤）
  const getServerMessages = (serverId: number): MqttMessage[] => {
    return messagesByServer.value.get(serverId) || [];
  };

  // 清空消息
  const clearMessages = (serverId?: number) => {
    const newMap = new Map(messagesByServer.value);
    if (serverId) {
      newMap.delete(serverId);
    } else {
      newMap.clear();
    }
    messagesByServer.value = newMap;
  };

  // 将 HEX 字符串转换为字节数组
  const hexToBytes = (hex: string): Uint8Array => {
    const cleanHex = hex.replace(/\s/g, "");
    const bytes = new Uint8Array(cleanHex.length / 2);
    for (let i = 0; i < cleanHex.length; i += 2) {
      bytes[i / 2] = parseInt(cleanHex.substring(i, i + 2), 16);
    }
    return bytes;
  };

  // 添加发布消息到列表（用于UI显示）
  const addPublishMessage = (
    serverId: number,
    msg: {
      topic: string;
      payload: string;
      qos: 0 | 1 | 2;
      retain: boolean;
      scriptError?: string;
      payload_type?: "json" | "hex" | "text";
    }
  ) => {
    // 根据 payload_type 决定如何编码 payload
    let payloadBytes: Uint8Array;
    if (msg.payload_type === "hex") {
      // HEX 格式：将 HEX 字符串转换为实际字节
      payloadBytes = hexToBytes(msg.payload);
    } else {
      // 其他格式：直接用 TextEncoder 编码
      payloadBytes = new TextEncoder().encode(msg.payload);
    }
    
    // 使用批处理队列
    queueMessage({
      server_id: serverId,
      direction: "publish",
      topic: msg.topic,
      payload: payloadBytes,
      qos: msg.qos,
      retain: msg.retain,
      timestamp: new Date().toISOString(),
      scriptError: msg.scriptError,
      payload_type: msg.payload_type,
    });
  };

  return {
    connectionStates,
    messagesByServer,
    subscriptions,
    initListeners,
    connect,
    disconnect,
    publish,
    subscribe,
    unsubscribe,
    getConnectionStatus,
    getConnectionError,
    getServerMessages,
    clearMessages,
    addPublishMessage,
    clearScriptCache,
    clearEnvCache,
  };
});
