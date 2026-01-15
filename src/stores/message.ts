import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MessageHistory, PublishPayload } from "@/types/mqtt";

export const useMessageStore = defineStore("message", () => {
  const messages = ref<Map<number, MessageHistory[]>>(new Map());
  const realtimeMessages = ref<Map<number, MessageHistory[]>>(new Map());
  const loading = ref(false);

  async function fetchMessageHistory(
    serverId: number,
    limit = 100,
    offset = 0
  ) {
    loading.value = true;
    try {
      const result = await invoke<MessageHistory[]>("get_message_history", {
        serverId,
        limit,
        offset,
      });
      messages.value.set(serverId, result);
    } finally {
      loading.value = false;
    }
  }

  async function publishMessage(serverId: number, message: PublishPayload) {
    const result = await invoke<MessageHistory>("publish_message", {
      serverId,
      message,
    });

    // 添加到消息列表
    addMessage(serverId, result);

    return result;
  }

  function addMessage(serverId: number, message: MessageHistory) {
    const serverMessages = messages.value.get(serverId) || [];
    serverMessages.unshift(message);

    // 限制消息数量
    if (serverMessages.length > 1000) {
      serverMessages.pop();
    }

    messages.value.set(serverId, serverMessages);

    // 同时更新实时消息列表
    const realtimeMsgs = realtimeMessages.value.get(serverId) || [];
    realtimeMsgs.unshift(message);
    if (realtimeMsgs.length > 100) {
      realtimeMsgs.pop();
    }
    realtimeMessages.value.set(serverId, realtimeMsgs);
  }

  async function clearHistory(serverId: number) {
    await invoke("clear_message_history", { serverId });
    messages.value.set(serverId, []);
    realtimeMessages.value.set(serverId, []);
  }

  function getMessages(serverId: number) {
    return messages.value.get(serverId) || [];
  }

  function getRealtimeMessages(serverId: number) {
    return realtimeMessages.value.get(serverId) || [];
  }

  return {
    messages,
    realtimeMessages,
    loading,
    fetchMessageHistory,
    publishMessage,
    addMessage,
    clearHistory,
    getMessages,
    getRealtimeMessages,
  };
});
