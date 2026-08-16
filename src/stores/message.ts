import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MessageHistory, PublishPayload } from "@/types/mqtt";

export const useMessageStore = defineStore("message", () => {
  const loading = ref(false);

  async function publishMessage(serverId: number, message: PublishPayload) {
    return await invoke<MessageHistory>("publish_message", {
      serverId,
      message,
    });
  }

  return {
    loading,
    publishMessage,
  };
});
