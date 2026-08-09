import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MessageHistory, PublishPayload } from "@/types/mqtt";

export const useMessageStore = defineStore("message", () => {
  const loading = ref(false);

  async function fetchMessageHistory(
    serverId: number,
    limit = 100,
    offset = 0
  ): Promise<MessageHistory[]> {
    loading.value = true;
    try {
      return await invoke<MessageHistory[]>("get_message_history", {
        serverId,
        limit,
        offset,
      });
    } finally {
      loading.value = false;
    }
  }

  async function publishMessage(serverId: number, message: PublishPayload) {
    return await invoke<MessageHistory>("publish_message", {
      serverId,
      message,
    });
  }

  async function clearHistory(serverId: number) {
    await invoke("clear_message_history", { serverId });
  }

  return {
    loading,
    fetchMessageHistory,
    publishMessage,
    clearHistory,
  };
});
