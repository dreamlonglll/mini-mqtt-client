<template>
  <AppLayout>
    <MainContent />
  </AppLayout>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import AppLayout from "@/components/layout/AppLayout.vue";
import MainContent from "@/components/mqtt/MainContent.vue";
import { useAppStore } from "@/stores/app";
import { useMqttStore } from "@/stores/mqtt";

const appStore = useAppStore();
const mqttStore = useMqttStore();

onMounted(() => {
  // 初始化主题
  appStore.initTheme();
  // 初始化 MQTT 事件监听
  mqttStore.initListeners();
});
</script>

<style>
/* 可在此添加额外的全局样式 */
</style>
