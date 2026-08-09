import { defineStore } from "pinia";
import { ref, shallowRef, triggerRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import type { ConnectionStatus, MqttMessage, EnvVariable } from "@/types/mqtt";
import { ScriptEngine } from "@/utils/scriptEngine";
import type { Script } from "@/stores/script";
import { handleScriptError } from "@/utils/errorHandler";
import { computeDerived } from "@/utils/messageDerived";
import { useAppStore } from "@/stores/app";
import i18n from "@/i18n";

interface ConnectionState {
  server_id: number;
  status: ConnectionStatus;
  error?: string;
}

interface ReceivedMessage {
  server_id: number;
  topic: string;
  /** base64 编码的消息体（Rust 侧编码，避免 JSON 数字数组的体积膨胀） */
  payload: string;
  qos: number;
  retain: boolean;
  /** Unix 毫秒时间戳 */
  timestamp: number;
  /** 原始 payload 字节数 */
  original_length: number;
  /** payload 是否被后端截断 */
  truncated: boolean;
}

// base64 字符串解码为 Uint8Array
function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

// 脚本缓存条目（缓存 Promise 而非结果，TTL 到期瞬间的并发请求共享同一次 IPC，避免惊群）
interface ScriptCacheEntry {
  promise: Promise<Script[]>;
  /** promise 完成后同步可读的结果（用于无脚本同步快路径判断） */
  resolved?: Script[];
  timestamp: number;
}

// 环境变量缓存条目
interface EnvCacheEntry {
  promise: Promise<Record<string, string>>;
  timestamp: number;
}

// 脚本缓存有效期（毫秒）
const SCRIPT_CACHE_TTL = 5000;
// 消息批处理间隔（毫秒）
const BATCH_INTERVAL = 50;
// 队列背压阈值：达到即立即同步 flush，防止窗口最小化时定时器被节流导致队列无界增长
const MAX_QUEUE_LENGTH = 2000;

// 消息 ID 计数器（确保每条消息有唯一标识，供虚拟滚动使用）
let messageIdCounter = 0;

export const useMqttStore = defineStore("mqtt", () => {
  const appStore = useAppStore();

  // 连接状态
  const connectionStates = ref<
    Map<number, { status: ConnectionStatus; error?: string }>
  >(new Map());

  // 按 serverId 分组存储消息（使用 shallowRef 减少深度响应式开销）
  const messagesByServer = shallowRef<Map<number, MqttMessage[]>>(new Map());

  // 被裁剪掉的消息总数（供 MessageList 定期重置虚拟滚动的行高缓存，防止内存泄漏）
  const trimmedCount = ref(0);

  // 订阅列表（按 server_id 分组）
  const subscriptions = ref<Map<number, Set<string>>>(new Map());

  // 脚本缓存（避免高频调用 invoke）
  const scriptCache = new Map<string, ScriptCacheEntry>();

  // 环境变量缓存
  const envCache = new Map<number, EnvCacheEntry>();

  // 消息批处理队列
  const messageQueue: MqttMessage[] = [];
  let batchTimeout: ReturnType<typeof setTimeout> | null = null;

  // 获取缓存的脚本（Promise 缓存：miss 时先写入 pending Promise，并发请求共享）
  function getCachedScripts(serverId: number, scriptType: string): Promise<Script[]> {
    const cacheKey = `${serverId}-${scriptType}`;
    const now = Date.now();
    const cached = scriptCache.get(cacheKey);

    if (cached && now - cached.timestamp < SCRIPT_CACHE_TTL) {
      return cached.promise;
    }

    const entry: ScriptCacheEntry = { promise: Promise.resolve([]), timestamp: now };
    entry.promise = invoke<Script[]>("get_enabled_scripts", { serverId, scriptType })
      .then((scripts) => {
        entry.resolved = scripts;
        return scripts;
      })
      .catch(() => {
        // 失败不占用整个 TTL，下次调用重试
        if (scriptCache.get(cacheKey) === entry) {
          scriptCache.delete(cacheKey);
        }
        return [] as Script[];
      });
    scriptCache.set(cacheKey, entry);
    return entry.promise;
  }

  // 同步判断某 server 是否确定没有启用的接收脚本（缓存已解析且为空）
  function hasNoReceiveScriptsSync(serverId: number): boolean {
    const cached = scriptCache.get(`${serverId}-after_receive`);
    return (
      !!cached &&
      cached.resolved !== undefined &&
      cached.resolved.length === 0 &&
      Date.now() - cached.timestamp < SCRIPT_CACHE_TTL
    );
  }

  // 清除脚本缓存（当脚本更新时调用）
  function clearScriptCache(serverId?: number) {
    if (serverId) {
      scriptCache.delete(`${serverId}-before_publish`);
      scriptCache.delete(`${serverId}-after_receive`);
    } else {
      scriptCache.clear();
    }
    // 同步失效脚本引擎的编译缓存
    ScriptEngine.clearCompileCache();
  }

  // 获取缓存的环境变量（Promise 缓存，避免 TTL 到期瞬间惊群）
  function getCachedEnvVariables(serverId: number): Promise<Record<string, string>> {
    const now = Date.now();
    const cached = envCache.get(serverId);

    if (cached && now - cached.timestamp < SCRIPT_CACHE_TTL) {
      return cached.promise;
    }

    const entry: EnvCacheEntry = { promise: Promise.resolve({}), timestamp: now };
    entry.promise = invoke<EnvVariable[]>("list_env_variables", { serverId })
      .then((envList) => {
        const variables: Record<string, string> = {};
        for (const env of envList) {
          variables[env.name] = env.value;
        }
        return variables;
      })
      .catch(() => {
        if (envCache.get(serverId) === entry) {
          envCache.delete(serverId);
        }
        return {} as Record<string, string>;
      });
    envCache.set(serverId, entry);
    return entry.promise;
  }

  // 清除环境变量缓存
  function clearEnvCache(serverId?: number) {
    if (serverId) {
      envCache.delete(serverId);
    } else {
      envCache.clear();
    }
  }

  // 批量处理消息队列（就地增删 + triggerRef，避免整表拷贝）
  function flushMessageQueue() {
    if (batchTimeout) {
      clearTimeout(batchTimeout);
      batchTimeout = null;
    }
    if (messageQueue.length === 0) return;

    const map = messagesByServer.value;

    // 按 serverId 分组
    const grouped = new Map<number, MqttMessage[]>();
    for (const msg of messageQueue) {
      let arr = grouped.get(msg.server_id);
      if (!arr) {
        arr = [];
        grouped.set(msg.server_id, arr);
      }
      arr.push(msg);
    }

    const limit = appStore.messageLimit;
    for (const [serverId, newMessages] of grouped) {
      let list = map.get(serverId);
      if (!list) {
        list = [];
        map.set(serverId, list);
      }
      // 就地头插 + 截断（虚拟滚动的更新由 triggerRef 通知）
      list.unshift(...newMessages);
      if (list.length > limit) {
        trimmedCount.value += list.length - limit;
        list.length = limit;
      }
    }

    messageQueue.length = 0;
    triggerRef(messagesByServer);
  }

  // 添加消息到队列
  function queueMessage(msg: MqttMessage) {
    msg.id = ++messageIdCounter;
    // 入队时一次性计算派生数据（decodedText / payloadFormat），渲染和搜索直接读缓存
    computeDerived(msg);
    messageQueue.push(msg);

    // 背压：队列达到阈值立即同步 flush
    if (messageQueue.length >= MAX_QUEUE_LENGTH) {
      flushMessageQueue();
      return;
    }

    if (!batchTimeout) {
      batchTimeout = setTimeout(flushMessageQueue, BATCH_INTERVAL);
    }
  }

  // ReceivedMessage → MqttMessage
  function toReceivedMqttMessage(
    msg: ReceivedMessage,
    payloadBytes: Uint8Array,
    scriptError?: string
  ): MqttMessage {
    return {
      server_id: msg.server_id,
      direction: "receive",
      topic: msg.topic,
      payload: payloadBytes,
      qos: msg.qos as 0 | 1 | 2,
      retain: msg.retain,
      timestamp: msg.timestamp,
      scriptError,
      originalLength: msg.original_length,
      truncated: msg.truncated,
    };
  }

  // 处理单条接收到的消息（可能执行接收后脚本）
  async function handleReceivedMessage(msg: ReceivedMessage) {
    let payloadBytes = base64ToBytes(msg.payload);
    let scriptError: string | undefined = undefined;

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

    queueMessage(toReceivedMqttMessage(msg, payloadBytes, scriptError));
  }

  // 接收路径的串行链：保证配置脚本时消息不乱序（慢脚本不会被后到的快消息超车）
  let receiveChain: Promise<void> = Promise.resolve();
  let receiveChainPending = 0;

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

    // 监听接收消息（Rust 侧攒批 emit，一次事件携带一批消息）
    await listen<ReceivedMessage[]>("mqtt-messages", (event) => {
      const batch = event.payload;

      // 同步快路径：确定无接收脚本且串行链空闲时直接同步入队，
      // 零 microtask 开销且天然保序
      if (
        receiveChainPending === 0 &&
        batch.every((m) => hasNoReceiveScriptsSync(m.server_id))
      ) {
        for (const msg of batch) {
          queueMessage(toReceivedMqttMessage(msg, base64ToBytes(msg.payload)));
        }
        return;
      }

      // 慢路径：挂到串行链上按序处理
      receiveChainPending++;
      receiveChain = receiveChain
        .then(async () => {
          for (const msg of batch) {
            await handleReceivedMessage(msg);
          }
        })
        .finally(() => {
          receiveChainPending--;
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
      timestamp: Date.now(),
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
    const map = messagesByServer.value;
    if (serverId) {
      map.delete(serverId);
    } else {
      map.clear();
    }
    triggerRef(messagesByServer);
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
      timestamp: Date.now(),
      scriptError: msg.scriptError,
      payload_type: msg.payload_type,
    });
  };

  return {
    connectionStates,
    messagesByServer,
    trimmedCount,
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
    getCachedScripts,
    getCachedEnvVariables,
    clearScriptCache,
    clearEnvCache,
  };
});
