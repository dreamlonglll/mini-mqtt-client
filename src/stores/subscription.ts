import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Subscription, UpdateSubscriptionRequest } from "@/types/mqtt";
import { validateSubscribeTopic } from "@/utils/mqttErrorHandler";
import { useEnvStore } from "@/stores/env";

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
    // 获取环境变量并替换 topic 中的变量
    const envStore = useEnvStore();
    if (envStore.variables.length === 0) {
      await envStore.loadVariables(serverId);
    }
    const processedTopic = envStore.replaceVariables(topic);
    
    // 验证订阅 Topic（使用替换后的 topic）
    const validation = validateSubscribeTopic(processedTopic);
    if (!validation.valid) {
      throw new Error(validation.error || "Topic 格式无效");
    }

    const result = await invoke<Subscription>("add_subscription", {
      serverId,
      topic: processedTopic,
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

  async function updateSubscription(
    serverId: number,
    oldTopic: string,
    request: UpdateSubscriptionRequest
  ) {
    // 如果 topic 改变了，需要验证新 topic
    if (request.topic) {
      const validation = validateSubscribeTopic(request.topic);
      if (!validation.valid) {
        throw new Error(validation.error || "Topic 格式无效");
      }
    }

    const result = await invoke<Subscription>("update_subscription", {
      serverId,
      oldTopic,
      request,
    });

    const serverSubs = subscriptions.value.get(serverId) || [];
    const index = serverSubs.findIndex((s) => s.id === request.id);
    if (index !== -1) {
      serverSubs[index] = result;
      subscriptions.value.set(serverId, serverSubs);
    }

    return result;
  }

  // 根据 topic 获取订阅（用于消息列表查找颜色）
  function getSubscriptionByTopic(serverId: number, topic: string): Subscription | undefined {
    const serverSubs = subscriptions.value.get(serverId) || [];
    // 先精确匹配
    let sub = serverSubs.find((s) => s.topic === topic);
    if (sub) return sub;
    
    // 再模糊匹配（支持通配符）
    for (const s of serverSubs) {
      if (matchTopic(s.topic, topic)) {
        return s;
      }
    }
    return undefined;
  }

  // MQTT topic 通配符匹配
  function matchTopic(pattern: string, topic: string): boolean {
    // # 匹配任意层级
    if (pattern === "#") return true;
    
    const patternParts = pattern.split("/");
    const topicParts = topic.split("/");
    
    for (let i = 0; i < patternParts.length; i++) {
      const p = patternParts[i];
      
      // # 匹配剩余所有层级
      if (p === "#") return true;
      
      // + 匹配单层级
      if (p === "+") {
        if (i >= topicParts.length) return false;
        continue;
      }
      
      // 精确匹配
      if (p !== topicParts[i]) return false;
    }
    
    return patternParts.length === topicParts.length;
  }

  return {
    subscriptions,
    loading,
    getSubscriptionsByServer,
    fetchSubscriptions,
    addSubscription,
    removeSubscription,
    toggleSubscription,
    updateSubscription,
    getSubscriptionByTopic,
  };
});
