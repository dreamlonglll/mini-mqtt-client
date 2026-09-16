import { defineStore } from "pinia";
import { ref, shallowRef, triggerRef } from "vue";
import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import type { ConnectionStatus, MqttMessage, EnvVariable } from "@/types/mqtt";
import { ScriptEngine } from "@/utils/scriptEngine";
import type { Script } from "@/stores/script";
import { handleScriptError, handleMqttError } from "@/utils/errorHandler";
import { decodeMessageFrames, type ReceivedMessage } from "@/utils/messageFrame";
import { hexToBytes, utf8Decoder, utf8Encoder } from "@/utils/encoding";
import { useAppStore } from "@/stores/app";
import { useSubscriptionStore } from "@/stores/subscription";
import i18n from "@/i18n";

interface ConnectionState {
  server_id: number;
  status: ConnectionStatus;
  error?: string;
}

// 脚本缓存条目（缓存 Promise 而非结果，并发请求共享同一次 IPC，避免惊群）
//
// 缓存不设 TTL：本应用是脚本与环境变量的唯一写入方，脚本 / 环境变量的增删改
// 以及删除 Server 都会主动调用 clearScriptCache / clearEnvCache 使其失效。
// 之前的 5 秒 TTL 只会让消息流每 5 秒掉一次到慢路径并多打一次 IPC。
interface ScriptCacheEntry {
  promise: Promise<Script[]>;
  /** promise 完成后同步可读的结果（用于无脚本同步快路径判断） */
  resolved?: Script[];
}

// 环境变量缓存条目
interface EnvCacheEntry {
  promise: Promise<Record<string, string>>;
}

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

  // 消息列表变更版本号
  //
  // 每个 Server 的数组是就地增删的，`getServerMessages` 返回的始终是同一个数组实例；
  // Vue 的 computed 在返回值同一时不会通知下游，所以列表组件必须读取这个版本号
  // 才能在每次 flush 后重算。
  const messagesVersion = ref(0);

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
    const cached = scriptCache.get(cacheKey);
    if (cached) {
      return cached.promise;
    }

    const entry: ScriptCacheEntry = { promise: Promise.resolve([]) };
    entry.promise = invoke<Script[]>("get_enabled_scripts", { serverId, scriptType })
      .then((scripts) => {
        entry.resolved = scripts;
        return scripts;
      })
      .catch(() => {
        // 失败不留在缓存里，下次调用重试
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
    return !!cached && cached.resolved !== undefined && cached.resolved.length === 0;
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

  // 获取缓存的环境变量（Promise 缓存，并发请求共享）
  function getCachedEnvVariables(serverId: number): Promise<Record<string, string>> {
    const cached = envCache.get(serverId);
    if (cached) {
      return cached.promise;
    }

    const entry: EnvCacheEntry = { promise: Promise.resolve({}) };
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

  // 批量处理消息队列（就地增删 + 版本号通知，避免整表拷贝）
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
      // 就地头插 + 截断（批内反转，维持 index 0 = 最新的约定）
      list.unshift(...newMessages.reverse());
      if (list.length > limit) {
        list.length = limit;
      }
    }

    messageQueue.length = 0;
    triggerRef(messagesByServer);
    messagesVersion.value++;
  }

  // 添加消息到队列
  function queueMessage(msg: MqttMessage) {
    msg.id = ++messageIdCounter;
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
    scriptError?: string,
    decodedText?: string
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
      // 脚本已产出字符串时直接作为解码缓存，省一次 decode(encode(str))
      decodedText,
    };
  }

  // 处理单条接收到的消息（可能执行接收后脚本）
  async function handleReceivedMessage(msg: ReceivedMessage) {
    let payloadBytes = msg.payload;
    let decodedText: string | undefined;
    let scriptError: string | undefined;

    try {
      const scripts = await getCachedScripts(msg.server_id, "after_receive");

      if (scripts.length > 0) {
        const originalPayload = utf8Decoder.decode(payloadBytes);
        const envVariables = await getCachedEnvVariables(msg.server_id);
        const processedPayload = await ScriptEngine.executeAfterReceive(
          scripts,
          originalPayload,
          msg.topic,
          envVariables
        );
        payloadBytes = utf8Encoder.encode(processedPayload);
        decodedText = processedPayload;
      }
    } catch (error: any) {
      // 记录脚本错误
      scriptError = error?.message || String(error);
      handleScriptError(error, true); // 静默处理，不显示通知（会写入日志）
    }

    queueMessage(toReceivedMqttMessage(msg, payloadBytes, scriptError, decodedText));
  }

  // 接收路径的串行链：保证配置脚本时消息不乱序（慢脚本不会被后到的快消息超车）
  let receiveChain: Promise<void> = Promise.resolve();
  let receiveChainPending = 0;

  // 处理后端推来的一批消息
  function handleReceivedBatch(batch: ReceivedMessage[]) {
    // 同步快路径：确定无接收脚本且串行链空闲时直接同步入队，
    // 零 microtask 开销且天然保序
    if (
      receiveChainPending === 0 &&
      batch.every((m) => hasNoReceiveScriptsSync(m.server_id))
    ) {
      for (const msg of batch) {
        queueMessage(toReceivedMqttMessage(msg, msg.payload));
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
      // 单条消息处理失败时链必须恢复，否则后续消息会被静默丢弃
      .catch((error) => {
        handleMqttError(error, true);
      })
      .finally(() => {
        receiveChainPending--;
      });
  }

  // 正在恢复订阅的 Server，防止连续事件导致重复订阅
  const restoringServers = new Set<number>();

  /**
   * 恢复某个 Server 的全部活跃订阅
   *
   * 按事件携带的 server_id 处理，不依赖"当前活跃 Server"的派生状态，
   * 因此后台 Server 断线重连后订阅同样会被恢复。
   */
  async function restoreSubscriptions(serverId: number) {
    if (restoringServers.has(serverId)) return;
    restoringServers.add(serverId);
    try {
      const subscriptionStore = useSubscriptionStore();
      // 非活跃 Server 的订阅可能还没加载进 store，先按需拉取
      if (!subscriptionStore.subscriptions.has(serverId)) {
        await subscriptionStore.fetchSubscriptions(serverId);
      }
      const activeSubscriptions = subscriptionStore
        .getSubscriptionsByServer(serverId)
        .filter((sub) => sub.is_active);

      for (const sub of activeSubscriptions) {
        try {
          await subscribe(serverId, sub.topic, sub.qos as 0 | 1 | 2);
        } catch (e) {
          handleMqttError(String(e), true);
        }
      }
    } catch (e) {
      handleMqttError(String(e), true);
    } finally {
      restoringServers.delete(serverId);
    }
  }

  // 事件监听的注销函数（已初始化时非空，用作幂等守卫）
  let listenerUnsubscribers: UnlistenFn[] | null = null;
  // 接收消息的 Channel（后端把一批消息编成二进制帧经此推送）
  let messageChannel: Channel<ArrayBuffer> | null = null;
  // 初始化中的 Promise，防止并发调用产生双份监听
  let initListenersPromise: Promise<void> | null = null;

  // 初始化事件监听（幂等：重复调用不会产生双份监听）
  const initListeners = async (): Promise<void> => {
    if (listenerUnsubscribers) return;
    if (initListenersPromise) return initListenersPromise;

    initListenersPromise = doInitListeners()
      .catch((e) => {
        // 失败时允许下次重试
        initListenersPromise = null;
        throw e;
      })
      .finally(() => {
        initListenersPromise = null;
      });
    return initListenersPromise;
  };

  // 注销全部事件监听（供测试与热重载复位使用）
  const disposeListeners = () => {
    if (!listenerUnsubscribers) return;
    for (const unlisten of listenerUnsubscribers) {
      unlisten();
    }
    listenerUnsubscribers = null;
    // Channel 没有前端侧的注销 API：置空处理函数即可，
    // 下次注册时后端会替换并结束旧 Channel，其回调随之清理
    if (messageChannel) {
      messageChannel.onmessage = () => {};
      messageChannel = null;
    }
  };

  const doInitListeners = async () => {
    // 监听连接状态变化（低频，仍走事件）
    const unlistenConnectionState = await listen<ConnectionState>("mqtt-connection-state", (event) => {
      const { server_id, status, error } = event.payload;
      const previousStatus = connectionStates.value.get(server_id)?.status;
      connectionStates.value.set(server_id, {
        status: status as ConnectionStatus,
        error,
      });

      // 任意 Server 出现"非连接 → 已连接"转变时恢复其订阅
      // （重连后 broker 侧订阅已失效，且后台 Server 也必须恢复）
      if (status === "connected" && previousStatus !== "connected") {
        void restoreSubscriptions(server_id);
      }

      // 如果有错误，使用 ElMessage 显示
      if (error && status === "error") {
        ElMessage.error({
          message: `${i18n.global.t('errors.connectFailed')}: ${error}`,
          duration: 5000,
        });
      }
    });

    // 接收消息走 Channel：一帧携带一批消息的原始字节，
    // 相比事件机制免去 JSON 序列化与脚本求值，大帧由 Tauri 走 fetch 通道投递
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (data) => {
      let batch: ReceivedMessage[];
      try {
        batch = decodeMessageFrames(data);
      } catch (error) {
        // 帧损坏：整帧丢弃并记日志，不影响后续批次
        handleMqttError(error, true);
        return;
      }
      handleReceivedBatch(batch);
    };
    try {
      await invoke("register_message_channel", { channel });
    } catch (error) {
      unlistenConnectionState();
      throw error;
    }

    messageChannel = channel;
    listenerUnsubscribers = [unlistenConnectionState];
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

  // 订阅
  const subscribe = async (
    serverId: number,
    topic: string,
    qos: 0 | 1 | 2 = 0
  ) => {
    await invoke("mqtt_subscribe", { serverId, topic, qos });
  };

  // 取消订阅
  const unsubscribe = async (serverId: number, topic: string) => {
    await invoke("mqtt_unsubscribe", { serverId, topic });
  };

  // 获取连接状态
  const getConnectionStatus = (serverId: number): ConnectionStatus => {
    return connectionStates.value.get(serverId)?.status || "disconnected";
  };

  // 获取连接错误
  const getConnectionError = (serverId: number): string | undefined => {
    return connectionStates.value.get(serverId)?.error;
  };

  // 获取某个 server 的消息（同一数组实例就地更新，变化由 messagesVersion 通知）
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
    messagesVersion.value++;
  };

  /**
   * 清理某个 Server 在本 store 的全部残留状态
   *
   * 供 Server 删除后级联调用，避免连接状态、消息、脚本与环境变量缓存
   * 长期残留（ID 复用时还会读到旧 Server 的数据）。
   */
  const clearServerState = (serverId: number) => {
    connectionStates.value.delete(serverId);
    messagesByServer.value.delete(serverId);
    triggerRef(messagesByServer);
    messagesVersion.value++;
    clearScriptCache(serverId);
    clearEnvCache(serverId);
    // 丢弃队列中该 Server 尚未 flush 的消息
    for (let i = messageQueue.length - 1; i >= 0; i--) {
      if (messageQueue[i].server_id === serverId) {
        messageQueue.splice(i, 1);
      }
    }
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
    let decodedText: string | undefined;
    if (msg.payload_type === "hex") {
      // HEX 格式：将 HEX 字符串转换为实际字节
      try {
        payloadBytes = hexToBytes(msg.payload);
      } catch {
        // 非法 HEX（例如脚本失败时展示的未替换原文）按文本展示，
        // UI 入队不能因为展示用的解码失败而抛错
        payloadBytes = utf8Encoder.encode(msg.payload);
        decodedText = msg.payload;
      }
    } else {
      // 其他格式：直接用 TextEncoder 编码，原文即解码结果
      payloadBytes = utf8Encoder.encode(msg.payload);
      decodedText = msg.payload;
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
      decodedText,
    });
  };

  return {
    connectionStates,
    messagesByServer,
    messagesVersion,
    initListeners,
    disposeListeners,
    connect,
    disconnect,
    subscribe,
    unsubscribe,
    getConnectionStatus,
    getConnectionError,
    getServerMessages,
    clearMessages,
    clearServerState,
    addPublishMessage,
    getCachedScripts,
    getCachedEnvVariables,
    clearScriptCache,
    clearEnvCache,
  };
});
