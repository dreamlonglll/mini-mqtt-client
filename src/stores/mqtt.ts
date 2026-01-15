import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnectionStatus, MqttMessage } from "@/types/mqtt";

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

export const useMqttStore = defineStore("mqtt", () => {
  // 连接状态
  const connectionStates = ref<
    Map<number, { status: ConnectionStatus; error?: string }>
  >(new Map());

  // 接收到的消息
  const messages = ref<MqttMessage[]>([]);

  // 订阅列表（按 server_id 分组）
  const subscriptions = ref<Map<number, Set<string>>>(new Map());

  // 初始化事件监听
  const initListeners = async () => {
    // 监听连接状态变化
    await listen<ConnectionState>("mqtt-connection-state", (event) => {
      const { server_id, status, error } = event.payload;
      connectionStates.value.set(server_id, {
        status: status as ConnectionStatus,
        error,
      });
    });

    // 监听接收消息
    await listen<ReceivedMessage>("mqtt-message", (event) => {
      const msg = event.payload;
      messages.value.unshift({
        server_id: msg.server_id,
        direction: "receive",
        topic: msg.topic,
        payload: new Uint8Array(msg.payload),
        qos: msg.qos as 0 | 1 | 2,
        retain: msg.retain,
        timestamp: msg.timestamp,
      });

      // 限制消息数量
      if (messages.value.length > 1000) {
        messages.value = messages.value.slice(0, 1000);
      }
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

    // 添加到消息列表
    messages.value.unshift({
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

  // 获取某个 server 的消息
  const getServerMessages = (serverId: number) => {
    return messages.value.filter((m) => m.server_id === serverId);
  };

  // 清空消息
  const clearMessages = (serverId?: number) => {
    if (serverId) {
      messages.value = messages.value.filter((m) => m.server_id !== serverId);
    } else {
      messages.value = [];
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
    }
  ) => {
    messages.value.unshift({
      server_id: serverId,
      direction: "publish",
      topic: msg.topic,
      payload: new TextEncoder().encode(msg.payload),
      qos: msg.qos,
      retain: msg.retain,
      timestamp: new Date().toISOString(),
    });

    // 限制消息数量
    if (messages.value.length > 1000) {
      messages.value = messages.value.slice(0, 1000);
    }
  };

  return {
    connectionStates,
    messages,
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
  };
});
