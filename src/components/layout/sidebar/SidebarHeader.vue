<template>
  <div class="sidebar-header">
    <div class="logo">
      <div class="logo-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
        </svg>
      </div>
      <span class="logo-text">MQTT Client</span>
      <span
        class="version-tag"
        :class="{ 'has-update': appStore.updateInfo?.hasUpdate }"
        @click="handleVersionClick"
      >
        v{{ appVersion }}
        <span v-if="appStore.updateInfo?.hasUpdate" class="update-dot" />
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ElMessageBox } from "element-plus";
import { useAppStore, GITHUB_REPO } from "@/stores/app";

const { t } = useI18n();
const appStore = useAppStore();

const appVersion = ref("");

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "1.0.0";
  }
  // 启动时检查更新
  appStore.checkUpdate();
});

// 有新版本时点击版本号跳转到 release 页
const handleVersionClick = async () => {
  if (!appStore.updateInfo?.hasUpdate) return;
  try {
    await ElMessageBox.confirm(
      t('sidebar.update.confirmDownload', { version: appStore.updateInfo.latestVersion }),
      t('sidebar.update.newVersionFound'),
      {
        confirmButtonText: t('sidebar.update.goDownload'),
        cancelButtonText: t('common.cancel'),
        type: 'info',
      }
    );
    await openUrl(`https://github.com/${GITHUB_REPO}/releases/latest`);
  } catch {
    // 用户取消
  }
};
</script>

<style scoped lang="scss">
.sidebar-header {
  padding: 16px;
  border-bottom: 1px solid var(--app-border-color);
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
}

.logo-icon {
  width: 28px;
  height: 28px;
  color: var(--primary-color);

  svg {
    width: 100%;
    height: 100%;
  }
}

.logo-text {
  font-size: 16px;
  font-weight: 600;
  color: var(--app-text-color);
}

.version-tag {
  font-size: 11px;
  font-weight: 400;
  color: var(--app-text-secondary);
  margin-left: 4px;
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 4px;

  &.has-update {
    color: var(--el-color-primary);
    cursor: pointer;

    &:hover {
      text-decoration: underline;
    }
  }
}

.update-dot {
  width: 8px;
  height: 8px;
  background-color: var(--el-color-danger);
  border-radius: 50%;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.6;
    transform: scale(1.1);
  }
  100% {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
