import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Subscription } from "@/types/mqtt";
import { validateSubscribeTopic } from "@/utils/mqttErrorHandler";

export const useSubscriptionStore = defineStore("subscription", () => {
  const subscriptions = ref<Map<number, Subscription[]>>(new Map());
  const loading = ref(false);

  const getSubscriptionsByServer = computed(() => {
    return (serverId: number) => subscriptions.value.get(serverId) || [];
  });

  async function fetchSubscriptions(serverId: number) {
    loading.value = true;
    try {
      const result = await invoke<Subscription[]>("get_subscriptions", {
        serverId,
      });
      subscriptions.value.set(serverId, result);
    } finally {
      loading.value = false;
    }
  }

  async function addSubscription(serverId: number, topic: string, qos: number) {
    // 验证订阅 Topic
    const validation = validateSubscribeTopic(topic);
    if (!validation.valid) {
      throw new Error(validation.error || "Topic 格式无效");
    }

    const result = await invoke<Subscription>("add_subscription", {
      serverId,
      topic,
      qos,
    });

    const serverSubs = subscriptions.value.get(serverId) || [];
    serverSubs.unshift(result);
    subscriptions.value.set(serverId, serverSubs);

    return result;
  }

  async function removeSubscription(
    subscriptionId: number,
    serverId: number,
    topic: string
  ) {
    await invoke("remove_subscription", {
      subscriptionId,
      serverId,
      topic,
    });

    const serverSubs = subscriptions.value.get(serverId) || [];
    const index = serverSubs.findIndex((s) => s.id === subscriptionId);
    if (index !== -1) {
      serverSubs.splice(index, 1);
      subscriptions.value.set(serverId, serverSubs);
    }
  }

  async function toggleSubscription(
    subscriptionId: number,
    serverId: number,
    topic: string,
    qos: number,
    isActive: boolean
  ) {
    await invoke("toggle_subscription", {
      subscriptionId,
      serverId,
      topic,
      qos,
      isActive,
    });

    const serverSubs = subscriptions.value.get(serverId) || [];
    const sub = serverSubs.find((s) => s.id === subscriptionId);
    if (sub) {
      sub.is_active = isActive;
    }
  }

  return {
    subscriptions,
    loading,
    getSubscriptionsByServer,
    fetchSubscriptions,
    addSubscription,
    removeSubscription,
    toggleSubscription,
  };
});
