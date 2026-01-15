<template>
  <div class="header">
    <div class="header-left">
      <span v-if="activeServer" class="current-view">
        消息调试
      </span>
      <span v-else class="welcome-text">欢迎使用 Mini MQTT Client</span>
    </div>

    <div class="header-right">
      <el-tooltip content="帮助文档" placement="bottom">
        <el-button :icon="QuestionFilled" circle size="small" />
      </el-tooltip>
      <el-tooltip content="设置" placement="bottom">
        <el-button :icon="Setting" circle size="small" @click="handleSettings" />
      </el-tooltip>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { QuestionFilled, Setting } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/server";

const serverStore = useServerStore();

const activeServer = computed(() => serverStore.activeServer);

const handleSettings = () => {
  // TODO: 打开设置对话框
  console.log("Open settings");
};
</script>

<style scoped lang="scss">
.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 20px;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-bottom: 1px solid var(--app-border-color);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.current-view {
  font-size: 15px;
  font-weight: 500;
  color: var(--app-text-color);
}

.welcome-text {
  font-size: 14px;
  color: var(--app-text-secondary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
