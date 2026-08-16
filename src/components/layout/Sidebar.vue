<template>
  <div class="sidebar">
    <SidebarHeader />

    <div class="sidebar-content">
      <ServerSection />

      <el-divider v-if="serverStore.activeServer" />

      <SubscriptionSection v-if="serverStore.activeServer" />
    </div>

    <div class="sidebar-footer">
      <el-button text @click="appStore.toggleTheme" class="theme-btn">
        <el-icon>
          <Sunny v-if="appStore.theme === 'light'" />
          <Moon v-else-if="appStore.theme === 'dark'" />
          <Platform v-else />
        </el-icon>
        <span>{{ themeLabel }}</span>
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Moon, Sunny, Platform } from "@element-plus/icons-vue";
import { useAppStore } from "@/stores/app";
import { useServerStore } from "@/stores/server";
import SidebarHeader from "./sidebar/SidebarHeader.vue";
import ServerSection from "./sidebar/ServerSection.vue";
import SubscriptionSection from "./sidebar/SubscriptionSection.vue";

const { t } = useI18n();

const appStore = useAppStore();
const serverStore = useServerStore();

// 主题标签文字
const themeLabel = computed(() => {
  switch (appStore.theme) {
    case 'light':
      return t('sidebar.theme.light');
    case 'dark':
      return t('sidebar.theme.dark');
    case 'auto':
      return t('sidebar.theme.auto');
    default:
      return t('sidebar.theme.light');
  }
});
</script>

<style scoped lang="scss">
.sidebar {
  height: 100%;
  background-color: var(--sidebar-bg);
  border-right: 1px solid var(--app-border-color);
  display: flex;
  flex-direction: column;
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.sidebar-footer {
  padding: 8px 12px;
  border-top: 1px solid var(--app-border-color);
}

.theme-btn {
  width: 100%;
  justify-content: flex-start;
}

:deep(.el-divider) {
  margin: 8px 0;
}
</style>
