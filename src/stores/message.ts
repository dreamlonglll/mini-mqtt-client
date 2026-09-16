import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { PublishPayload } from "@/types/mqtt";

export const useMessageStore = defineStore("message", () => {
  const loading = ref(false);

  /** 交给后端发布；发布记录只保留在前端消息列表，后端不再留副本 */
  async function publishMessage(serverId: number, message: PublishPayload): Promise<void> {
    await invoke("publish_message", {
      serverId,
      message,
    });
  }

  return {
    loading,
    publishMessage,
  };
});
