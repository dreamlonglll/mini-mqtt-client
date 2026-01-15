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

const appStore = useAppStore();

onMounted(() => {
  // 初始化主题
  appStore.initTheme();
});
</script>

<style>
/* 可在此添加额外的全局样式 */
</style>
